use std::sync::Arc;

use anyhow::Result;
use indexmap::IndexMap;
use petri_utils::subscriber_list;
use tokio::sync::RwLock;

use crate::process::{OutputSubscriber, Process, StartInfo};

pub struct ProcessManager {
    handle: Handle,
}

#[derive(Clone)]
pub struct Handle {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    processes: RwLock<IndexMap<u32, Process>>,
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

    pub async fn shutdown(&self) {
        let processes = self.handle.inner.processes.read().await;
        for process in processes.values() {
            info!("killing process {}...", process.id());
            process.kill().await;
        }
    }
}

impl Handle {
    pub async fn add_process(&self, start_info: StartInfo) -> Result<u32> {
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

    pub async fn attach_output_channel(
        &self,
        id: u32,
        sender: OutputSubscriber,
    ) -> Option<subscriber_list::CancellationToken<OutputSubscriber>> {
        let processes = self.inner.processes.read().await;
        let Some(process) = processes.get(&id) else {
            return None;
        };
        Some(process.attach_output_channel(sender).await)
    }

    pub(crate) async fn handle_process_exit(&self, id: u32, exit_code: i32) {
        info!("process {id} exit with code {exit_code}");
        self.inner.processes.write().await.remove(&id);
    }
}
