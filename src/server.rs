use anyhow::Result;
use tokio::sync::watch;

use crate::control::Control;
use crate::proc_mgr::ProcessManager;

pub async fn run_server() {
    // TODO: configure logging.
    server_main().await.unwrap();
}

async fn server_main() -> Result<()> {
    let (shutdown_request_tx, mut shutdown_request_rx) = watch::channel(false);
    let process_manager = ProcessManager::new();
    let control = Control::new(process_manager.handle(), shutdown_request_tx)?;

    loop {
        shutdown_request_rx.changed().await?;
        let is_shutdown_requested = *shutdown_request_rx.borrow_and_update();
        if is_shutdown_requested {
            println!("client requested to shutdown the server");
            break;
        }
    }

    println!("shutting down...");
    control.shutdown().await;
    process_manager.shutdown().await;

    println!("bye!");

    Ok(())
}
