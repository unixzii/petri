use std::fs;

use anyhow::Result;
use tokio::sync::watch;

use crate::control::Control;
use crate::logger::LoggerBuilder;
use crate::proc_mgr::ProcessManager;

pub async fn run_server() {
    configure_logger();
    if let Err(err) = server_main().await {
        error!("error occurred while the server is running:\n{err:?}");
        std::process::abort();
    }
}

#[inline(always)]
fn configure_logger() {
    let mut logger = LoggerBuilder::new();

    if let Some(home_dir) = home::home_dir() {
        let mut logs_dir = home_dir;
        logs_dir.push(".petri");
        logs_dir.push("logs");
        if logs_dir.exists() || fs::create_dir_all(&logs_dir).is_ok() {
            logger = logger.enable_file(logs_dir);
        }
    }

    logger = logger.enable_stderr();

    if cfg!(debug_assertions) {
        log::set_max_level(log::LevelFilter::Trace);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }
    let boxed_logger = Box::new(logger.build());
    log::set_boxed_logger(boxed_logger).expect("failed to init logger");
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
