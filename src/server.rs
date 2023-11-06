use std::fs;

use anyhow::Result;
use tokio::sync::watch;

use crate::control::Control;
use crate::logger::LoggerBuilder;
use crate::proc_mgr::ProcessManager;

pub async fn run_server() {
    configure_logger();
    configure_panic_handler();
    if let Err(err) = server_main().await {
        panic!("error occurred while the server is running:\n{err:?}");
    }

    // Logs are written in a background thread. Wait for it to flush all
    // the pending contents before the process exits.
    ensure_logs_flushed();
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

fn ensure_logs_flushed() {
    log::logger().flush();
}

/// Installs a panic hook to write the panic info to disk.
///
/// This is necessary because the default panic handler that `std` provides
/// will only print the panic info to `stderr`. The server process typically
/// runs in background and no console is attached, so we need to construct
/// and write the log in our custom panic handler.
#[inline(always)]
fn configure_panic_handler() {
    use std::backtrace;
    use std::panic;
    use std::thread;

    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .expect("current implementation should always return `Some`");

        let msg = 'b: {
            if let Some(s) = info.payload().downcast_ref::<&'static str>() {
                break 'b *s;
            }

            match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "<message is not displayable>",
            }
        };

        let panicked_thread = thread::current();

        error!(
            "thread '{}' panicked at {location}:\n{msg}\n\nStack backtrace:\n{}",
            panicked_thread.name().unwrap_or("<unnamed>"),
            backtrace::Backtrace::force_capture()
        );
        ensure_logs_flushed();

        orig_hook(info)
    }));
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
