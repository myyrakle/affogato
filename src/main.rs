mod cli;
mod shutdown;

#[cfg(target_os = "linux")]
mod socket;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{HeaderMap, Request, Response};
use hyper_util::rt::TokioIo;
use socket::{FileDescriptors, FileDescriptorsMap};
use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::os::fd::FromRawFd;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpSocket};
use tokio::sync::Mutex;

const PROXY_HOST_HEADER: &str = "Proxy-Host";
const UPGRADE_SOCKET_PATH: &str = "/tmp/affogato_upgrade.sock";

async fn handle_proxy_request(
    mut request: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    // 1. get 'Proxy-Host' header from request
    let headers = request.headers_mut();

    let Some(proxy_target) = headers.remove(PROXY_HOST_HEADER) else {
        return Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("Proxy-Host header is missing")))
            .unwrap());
    };

    let Ok(proxy_target) = proxy_target.to_str() else {
        return Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from(
                "Proxy-Host header is not a valid string",
            )))
            .unwrap());
    };

    // 2. prepare request

    // 2.1. get request method
    let method = request.method().to_owned();

    // 2.2. get request headers
    let mut request_headers = HeaderMap::new();
    std::mem::swap(&mut request_headers, request.headers_mut());

    // 2.3 generate request URI for proxy
    let request_uri = {
        let uri = request.uri();
        let path = uri.path();
        let raw_query = uri.query();
        let mut request_uri =
            String::with_capacity(proxy_target.len() + path.len() + raw_query.unwrap_or("").len());

        request_uri.push_str(proxy_target);
        request_uri.push_str(path);

        if let Some(raw_query) = raw_query {
            request_uri.push('?');
            request_uri.push_str(raw_query);
        }

        request_uri
    };

    // 2.4. get request body
    let Ok(request_body) = request.into_body().collect().await.map(|body| {
        let bytes = body.to_bytes().to_vec();
        unsafe { String::from_utf8_unchecked(bytes) }
    }) else {
        return Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("Failed to read request body")))
            .unwrap());
    };

    // 3. send request to proxy
    let Ok(client) = reqwest::ClientBuilder::new().build() else {
        return Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("Failed to create a reqwest client")))
            .unwrap());
    };

    let proxy_request = client
        .request(method, request_uri)
        .body(request_body)
        .headers(request_headers);

    let proxy_result = proxy_request.send().await;

    // 4. return response from proxy to client
    match proxy_result {
        Ok(response) => {
            let mut response_builder = Response::builder().status(response.status());

            let headers = response_builder.headers_mut().unwrap();

            for (key, value) in response.headers() {
                headers.insert(key, value.clone());
            }

            let body = response.bytes().await.unwrap();

            Ok(response_builder.body(Full::new(body)).unwrap())
        }
        Err(error) => Ok(Response::builder()
            .status(500)
            .body(Full::new(Bytes::from(format!(
                "Failed to send request: {error:?}",
            ))))
            .unwrap()),
    }
}

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
                    .serve_connection(io, service_fn(handle_proxy_request))
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    });

    // waiting for exit signal
    use tokio::signal::unix;

    let mut sigquit_signal = unix::signal(unix::SignalKind::quit()).unwrap();
    let mut sigterm_signal = unix::signal(unix::SignalKind::terminate()).unwrap();
    let mut sigint_signal = unix::signal(unix::SignalKind::interrupt()).unwrap();

    let shutdown_type = tokio::select! {
        _ = sigquit_signal.recv() => {
            log::info!("Received SIGQUIT signal");
            shutdown::ShutdownType::Graceful
        }
        _ = sigterm_signal.recv() => {
            log::info!("Received SIGTERM signal");
            shutdown::ShutdownType::Graceful
        }
        _ = sigint_signal.recv() => {
            log::info!("Received SIGINT signal");
            shutdown::ShutdownType::Immediate
        }
    };

    match shutdown_type {
        shutdown::ShutdownType::Immediate => {
            std::process::exit(0);
        }
        shutdown::ShutdownType::Graceful => {
            log::info!("Graceful shutdown started");
            std::thread::sleep(std::time::Duration::from_secs(5));

            #[cfg(target_os = "linux")]
            {
                let file_descriptors = file_descriptors.lock().await;

                file_descriptors
                    .block_socket_and_send_to_new_server(UPGRADE_SOCKET_PATH)
                    .expect("Failed to send file descriptors to new server");
            }

            log::info!("Graceful shutdown completed");
            std::process::exit(0);
        }
    }
}
