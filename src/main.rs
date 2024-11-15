mod cli;
pub mod constants;
mod proxy;
mod shutdown;
mod socket;

use constants::UPGRADE_SOCKET_PATH;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use socket::{FileDescriptors, FileDescriptorsMap};
use std::net::{IpAddr, SocketAddr};
use std::os::fd::FromRawFd;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpSocket};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    env_logger::init();

    let command = cli::parse_command();
    log::debug!("{:?}", command.value);

    // create address from command line arguments
    let port = command.value.port;
    let address = IpAddr::from_str(&command.value.address).unwrap();
    let host = SocketAddr::from((address, port));

    let file_descriptors: FileDescriptors = Arc::new(Mutex::new(FileDescriptorsMap::new()));

    if command.value.is_uprade_mode() {
        log::info!("Upgrade mode is enabled");

        // get file descriptors from the .sock file
        let mut file_descriptors = file_descriptors.lock().await;
        file_descriptors
            .get_from_sock(UPGRADE_SOCKET_PATH)
            .expect("Failed to get file descriptors from socket");
    }

    // create TCP listener bound to the address
    let listener = if command.value.is_uprade_mode() {
        let addr = host.to_string();

        let Some(fd) = file_descriptors
            .lock()
            .await
            .get(addr.as_str())
            .map(|e| e.to_owned())
        else {
            log::error!("Failed to get file descriptors from socket");
            std::process::exit(1);
        };

        let std_listener_stream = unsafe { std::net::TcpStream::from_raw_fd(fd) };

        let tcp_socket = TcpSocket::from_std_stream(std_listener_stream);

        tcp_socket.listen(65535).unwrap()
    } else {
        let listener = TcpListener::bind(host).await.unwrap();

        file_descriptors.lock().await.add(
            host.to_string(),
            std::os::unix::io::AsRawFd::as_raw_fd(&listener),
        );

        listener
    };

    // server thread
    // create TCP listener bound to the address
    tokio::spawn(async move {
        log::info!("Listening on http://{}", host);

        // main loop
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };

            let io = TokioIo::new(stream);

            // Spawn a tokio task to serve multiple connections concurrently
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service_fn(proxy::handle_proxy_request))
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    });

    // waiting for exit signal
    shutdown::handle_shutdown(file_descriptors).await;

    return Ok(());
}
