#[macro_use]
extern crate log;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Result;
use petri_control::Control;
use petri_core::process_mgr::ProcessManager;
use pin_project_lite::pin_project;
use tokio::sync::watch;

pin_project! {
    pub struct Server {
        fut: Pin<Box<dyn Future<Output = Result<()>>>>,
        drop_guard: DropGuard,
    }
}

impl Server {
    pub async fn new() -> Result<Self> {
        let (shutdown_request_tx, mut shutdown_request_rx) = watch::channel(false);
        let process_manager = ProcessManager::new();
        let control = Control::new(process_manager.handle(), shutdown_request_tx)?;

        info!("the server is started!");

        let dropped = Arc::new(AtomicBool::new(false));
        let drop_guard = DropGuard {
            dropped: Arc::clone(&dropped),
        };
        let fut = Box::pin(async move {
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

            dropped.store(true, AtomicOrdering::Relaxed);

            Ok(())
        });

        Ok(Self { fut, drop_guard })
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
