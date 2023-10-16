use anyhow::Result;
use tokio::sync::mpsc;

use crate::control::{Control, Message as ControlMessage};
use crate::proc_mgr::ProcessManager;

pub async fn run_server() {
    // TODO: configure logging.
    server_main().await.unwrap();
}

async fn server_main() -> Result<()> {
    let (message_tx, mut message_rx) = mpsc::channel(8);
    let process_manager = ProcessManager::new();
    let control = Control::new(process_manager.handle(), message_tx)?;

    // TODO: remove this attribute when adding more branches.
    #[allow(clippy::never_loop)]
    while let Some(message) = message_rx.recv().await {
        match message {
            ControlMessage::RequestShutdown => {
                println!("client requested to shutdown the server");
                break;
            }
        }
    }

    println!("shutting down...");
    control.shutdown().await;
    process_manager.shutdown().await;

    println!("bye!");

    Ok(())
}
