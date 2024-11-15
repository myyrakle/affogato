use crate::{constants::UPGRADE_SOCKET_PATH, socket::FileDescriptors};

pub enum ShutdownType {
    Immediate,
    Graceful,
}

pub async fn handle_shutdown(file_descriptors: FileDescriptors) {
    use tokio::signal::unix;

    let mut sigquit_signal = unix::signal(unix::SignalKind::quit()).unwrap();
    let mut sigterm_signal = unix::signal(unix::SignalKind::terminate()).unwrap();
    let mut sigint_signal = unix::signal(unix::SignalKind::interrupt()).unwrap();

    let shutdown_type = tokio::select! {
        _ = sigquit_signal.recv() => {
            log::info!("Received SIGQUIT signal");
            ShutdownType::Graceful
        }
        _ = sigterm_signal.recv() => {
            log::info!("Received SIGTERM signal");
            ShutdownType::Graceful
        }
        _ = sigint_signal.recv() => {
            log::info!("Received SIGINT signal");
            ShutdownType::Immediate
        }
    };

    match shutdown_type {
        ShutdownType::Immediate => {
            std::process::exit(0);
        }
        ShutdownType::Graceful => {
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
