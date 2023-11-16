use std::fs;
use std::time::Duration;

use petri_logger::LoggerBuilder;
use petri_server::Server;
use tokio::task as tokio_task;

use crate::logging;

pub async fn run_server() {
    configure_logger();
    configure_panic_handler();

    let server = match Server::new() {
        Ok(server) => server,
        Err(err) => panic!("failed to start the server:\n{err:?}"),
    };

    server.with_process_manager(|proc_mgr| {
        let driver = logging::rotation_callback_registry().make_driver();
        proc_mgr.set_logger_rotation_driver(driver);
    });

    if let Err(err) = server.await {
        panic!("error occurred while waiting the server:\n{err:?}");
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
            let registry = logging::rotation_callback_registry();
            let driver = registry.make_driver();
            logger = logger
                .enable_file(logs_dir)
                .enable_file_rotation(driver)
                .expect("this operation should never fail");
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

    // Start a timer to drive the log rotation checks.
    tokio_task::spawn(async {
        loop {
            if cfg!(debug_assertions) {
                tokio::time::sleep(Duration::from_secs(5)).await;
            } else {
                tokio::time::sleep(Duration::from_secs(30)).await;
            }

            logging::rotation_callback_registry().notify_all();
        }
    });
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
