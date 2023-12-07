use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::Duration;

use tokio::runtime::Handle as TokioHandle;
use tokio::sync::oneshot;
use tokio::time::sleep;

/// A task that is delayed for a specified duration.
pub struct DelayedTask {
    cancelled: Arc<AtomicBool>,
    cancelled_tx: Option<oneshot::Sender<()>>,
}

impl DelayedTask {
    /// Schedules a task delayed until `duration` has elapsed.
    pub fn schedule<F>(f: F, duration: Duration) -> Self
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        Self::schedule_in(f, duration, &TokioHandle::current())
    }

    /// Schedules a task delayed until `duration` has elapsed, in the runtime
    /// held by `handle`.
    pub fn schedule_in<F>(f: F, duration: Duration, handle: &TokioHandle) -> Self
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancelled_clone = Arc::clone(&cancelled);

        let fut = sleep(duration);
        let (tx, rx) = oneshot::channel();
        handle.spawn(async move {
            let fired = tokio::select! {
                _ = rx => { false },
                _ = fut => { true }
            };
            let cancelled = cancelled_clone.load(AtomicOrdering::Relaxed);
            if fired && !cancelled {
                f();
            }
        });
        Self {
            cancelled,
            cancelled_tx: Some(tx),
        }
    }

    /// Cancels the task if it's not executed.
    pub fn cancel(&mut self) {
        if let Some(tx) = self.cancelled_tx.take() {
            self.cancelled.store(true, AtomicOrdering::Relaxed);
            // It's ok if we failed to send the signal, since
            // the timer may have fired before we cancel.
            _ = tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::DelayedTask;

    #[tokio::test(start_paused = true)]
    async fn test_fire() {
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        let _delay = DelayedTask::schedule(
            move || fired_clone.store(true, AtomicOrdering::Relaxed),
            Duration::from_secs(1),
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!fired.load(AtomicOrdering::Relaxed));

        tokio::time::sleep(Duration::from_secs(10)).await;
        assert!(fired.load(AtomicOrdering::Relaxed));
    }

    #[tokio::test(start_paused = true)]
    async fn test_cancellation() {
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        let mut delay = DelayedTask::schedule(
            move || fired_clone.store(true, AtomicOrdering::Relaxed),
            Duration::from_secs(1),
        );

        delay.cancel();

        tokio::time::sleep(Duration::from_secs(10)).await;
        assert!(!fired.load(AtomicOrdering::Relaxed));
    }
}
