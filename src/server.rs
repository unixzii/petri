use anyhow::Result;

use crate::control::Control;
use crate::proc_mgr::ProcessManager;

pub async fn run_server() {
    // TODO: configure logging.
    server_main().await.unwrap();
}

async fn server_main() -> Result<()> {
    let process_manager = ProcessManager::new();
    let control = Control::new(process_manager.handle())?;

    #[cfg(debug_assertions)]
    prototype_keep_alive().await;

    println!("shutting down...");
    control.shutdown().await;
    process_manager.shutdown().await;

    println!("bye!");

    Ok(())
}

#[cfg(debug_assertions)]
async fn prototype_keep_alive() {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        println!("still alive");
    }
}
