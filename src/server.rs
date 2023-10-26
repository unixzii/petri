use anyhow::Result;
use tokio::sync::watch;

use crate::control::Control;
use crate::logger::LoggerBuilder;
use crate::proc_mgr::ProcessManager;

pub async fn run_server() {
    let logger = LoggerBuilder::new().enable_stderr().build();
    if cfg!(debug_assertions) {
        log::set_max_level(log::LevelFilter::Trace);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }
    log::set_boxed_logger(Box::new(logger)).expect("failed to init logger");

    // TODO: configure logging.
    server_main().await.unwrap();
}

async fn server_main() -> Result<()> {
    let (shutdown_request_tx, mut shutdown_request_rx) = watch::channel(false);
    let process_manager = ProcessManager::new();
    let control = Control::new(process_manager.handle(), shutdown_request_tx)?;

    info!("the server is started!");

    loop {
        shutdown_request_rx.changed().await?;
        let is_shutdown_requested = *shutdown_request_rx.borrow_and_update();
        if is_shutdown_requested {
            info!("client requested to shutdown the server");
            break;
        }
    }

    info!("the server is shutting down...");
    control.shutdown().await;
    process_manager.shutdown().await;

    info!("bye!");

    Ok(())
}
