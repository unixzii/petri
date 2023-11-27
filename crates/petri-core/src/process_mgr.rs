use std::sync::Arc;

use anyhow::Result;
use indexmap::IndexMap;
use parking_lot::Mutex;
use petri_logger::writers::file_writer::RotationDriver;
use petri_utils::subscriber_list::{CancellationToken, SubscriberList};
use tokio::sync::RwLock;

use crate::process::{OutputSubscriber, Process, StartInfo};

pub struct ProcessManager {
    handle: Handle,
}

#[derive(Clone)]
pub struct Handle {
    inner: Arc<Inner>,
}

pub trait EventHandler: Send + Sync {
    fn handle_process_exit(&self, pid: u32, exit_code: i32) {
        _ = pid;
        _ = exit_code;
    }
}

#[derive(Default)]
struct Inner {
    processes: RwLock<IndexMap<u32, Process>>,
    rotation_driver: Mutex<Option<Arc<dyn RotationDriver>>>,
    event_handlers: SubscriberList<Box<dyn EventHandler>>,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            handle: Handle {
                inner: Default::default(),
            },
        }
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    pub fn set_logger_rotation_driver<D>(&self, driver: D)
    where
        D: RotationDriver + 'static,
    {
        let mut rotation_driver = self.handle.inner.rotation_driver.lock();
        *rotation_driver = Some(Arc::new(driver));
    }

    pub async fn shutdown(&self) {
        let processes = self.handle.inner.processes.read().await;
        for process in processes.values() {
            info!("killing process {}...", process.id());
            process.kill().await;
        }
    }
}

impl Handle {
    pub async fn add_process(&self, start_info: &StartInfo) -> Result<u32> {
        let process = Process::spawn(&start_info, self)?;

        let id = process.id();
        self.inner.processes.write().await.insert(id, process);

        info!("process `{}` started (pid: {id})", start_info.program);

        Ok(id)
    }

    pub async fn stop_process(&self, id: u32) -> Result<i32> {
        let processes = self.inner.processes.read().await;
        let Some(process) = processes.get(&id) else {
            return Err(anyhow!("process with id `{id}` is not found"));
        };

        Ok(process.kill().await)
    }

    pub async fn processes(&self) -> Vec<Process> {
        let processes = self.inner.processes.read().await;
        processes.values().cloned().collect()
    }

    pub async fn process_with_id(&self, id: u32) -> Option<Process> {
        let processes = self.inner.processes.read().await;
        processes.get(&id).cloned()
    }

    pub async fn attach_output_channel(
        &self,
        id: u32,
        sender: OutputSubscriber,
    ) -> Option<CancellationToken<OutputSubscriber>> {
        let processes = self.inner.processes.read().await;
        let Some(process) = processes.get(&id) else {
            return None;
        };
        Some(process.attach_output_channel(sender).await)
    }

    pub fn add_event_handler<H: EventHandler + 'static>(
        &self,
        handler: H,
    ) -> CancellationToken<Box<dyn EventHandler>> {
        self.inner.event_handlers.subscribe(Box::new(handler))
    }

    pub(crate) async fn handle_process_exit(&self, id: u32, exit_code: i32) {
        info!("process {id} exit with code {exit_code}");

        let mut processes = self.inner.processes.write().await;
        processes.remove(&id);
        drop(processes);

        self.inner.event_handlers.for_each(|handler| {
            handler.handle_process_exit(id, exit_code);
        });
    }

    #[rustfmt::skip]
    pub(crate) fn logger_rotation_driver(&self) -> Option<Arc<dyn RotationDriver>> {
        self.inner.rotation_driver.lock().as_ref().map(Arc::clone)
    }
}
