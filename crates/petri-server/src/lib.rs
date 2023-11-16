#[macro_use]
extern crate log;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Result;
use parking_lot::Mutex;
use petri_core::process_mgr::ProcessManager;
use pin_project_lite::pin_project;
use tokio::sync::watch;

pin_project! {
    pub struct Server {
        fut: Pin<Box<dyn Future<Output = Result<()>>>>,
        process_manager: Arc<Mutex<Option<ProcessManager>>>,
        drop_guard: DropGuard,
    }
}

async fn wait_for_shutdown(mut shutdown_request_rx: watch::Receiver<bool>) {
    loop {
        if shutdown_request_rx.changed().await.is_err() {
            // Sender is dropped, so assume that we need to shutdown.
            break;
        }
        let is_shutdown_requested = *shutdown_request_rx.borrow_and_update();
        if is_shutdown_requested {
            break;
        }
    }
}

impl Server {
    pub fn new() -> Result<Self> {
        let (shutdown_request_tx, shutdown_request_rx) = watch::channel(false);
        let process_manager = ProcessManager::new();
        let proc_mgr_handle = process_manager.handle();

        // Wrap the process manager into a shared container, because the caller
        // may configure it before the future actually takes it.
        let process_manager = Arc::new(Mutex::new(Some(process_manager)));

        let process_manager_clone = Arc::clone(&process_manager);
        let dropped = Arc::new(AtomicBool::new(false));
        let drop_guard = DropGuard {
            dropped: Arc::clone(&dropped),
        };
        let fut = Box::pin(async move {
            let process_manager = process_manager_clone
                .lock()
                .take()
                .expect("process manager should be present before polling");

            info!("the server is started!");

            let control_ctx = petri_control::Context {
                proc_mgr_handle,
                shutdown_request: shutdown_request_tx,
            };

            tokio::select! {
                res = petri_control::run_control_server(control_ctx) => {
                    match res {
                        Ok(()) => {
                            unreachable!("this function should not return");
                        },
                        Err(e) => {
                            error!("error occurred while running control: {e:?}");
                        }
                    }
                },
                _ = wait_for_shutdown(shutdown_request_rx) => {
                    info!("client requested to shutdown the server");
                },
            }

            info!("the server is shutting down...");
            process_manager.shutdown().await;

            dropped.store(true, AtomicOrdering::Relaxed);

            info!("the server did shutdown successfully");

            Ok(())
        });

        Ok(Self {
            fut,
            process_manager,
            drop_guard,
        })
    }

    pub fn with_process_manager<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ProcessManager) -> R,
    {
        let process_manager = self.process_manager.lock();
        f(process_manager
            .as_ref()
            .expect("should not call this method after polling the server"))
    }
}

impl Future for Server {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let fut = this.fut.as_mut();
        fut.poll(cx)
    }
}

struct DropGuard {
    dropped: Arc<AtomicBool>,
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        if !self.dropped.load(AtomicOrdering::Relaxed) {
            // TODO: shutdown the server in the background.
            warn!("the server is not awaited before being dropped");
        }
    }
}
