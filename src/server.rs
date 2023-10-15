use std::io::Write;

use anyhow::Result;

use crate::control::Control;
use crate::proc_mgr::{ProcessManager, StartInfo};

pub async fn run_server() {
    // TODO: configure logging.
    server_main().await.unwrap();
}

async fn server_main() -> Result<()> {
    let process_manager = ProcessManager::new();
    let control = Control::new()?;

    #[cfg(debug_assertions)]
    test(&process_manager).await;

    println!("shutting down...");
    control.shutdown().await;
    process_manager.shutdown().await;

    println!("bye!");

    Ok(())
}

#[cfg(debug_assertions)]
async fn test(process_manager: &ProcessManager) {
    process_manager
        .add_process(StartInfo {
            program: "node".to_owned(),
            args: None,
            cwd: std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string(),
        })
        .await
        .unwrap();

    process_manager
        .add_process(StartInfo {
            program: "/Applications/AppCleaner.app/Contents/MacOS/AppCleaner".to_owned(),
            args: None,
            cwd: "/Users/cyandev".to_owned(),
        })
        .await
        .unwrap();

    let id = process_manager
        .add_process(StartInfo {
            program: "node".to_owned(),
            args: Some(vec!["/var/tmp/print.js".to_owned()]),
            cwd: std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string(),
        })
        .await
        .unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let _drop = process_manager.attach_output_channel(id, tx).await;

    while let Some(x) = rx.recv().await {
        std::io::stdout().write_all(&x).unwrap();
    }

    println!("log redirecting stopped");
}
