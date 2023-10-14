mod process;

use std::sync::Arc;

use anyhow::Result;
use indexmap::IndexMap;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;

use crate::util::subscriber_list;
pub use process::{Process, StartInfo};

pub struct ProcessManager {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    processes: RwLock<IndexMap<u32, Process>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub async fn add_process(&self, start_info: StartInfo) -> Result<u32> {
        let process = Process::spawn(&start_info, self)?;

        let id = process.id();
        self.inner.processes.write().await.insert(id, process);

        Ok(id)
    }

    pub async fn attach_output_channel(
        &self,
        id: u32,
        sender: UnboundedSender<Vec<u8>>,
    ) -> Option<subscriber_list::CancellationToken<UnboundedSender<Vec<u8>>>> {
        let processes = self.inner.processes.read().await;
        let Some(process) = processes.get(&id) else {
            return None;
        };
        Some(process.attach_output_channel(sender).await)
    }

    pub async fn shutdown(&self) {
        let processes = self.inner.processes.read().await;
        for process in processes.values() {
            println!("killing process {}...", process.id());
            process.kill().await;
        }
    }
}

impl Inner {
    async fn handle_process_exit(&self, id: u32, exit_code: i32) {
        println!("process {id} exit with code {exit_code}");
        self.processes.write().await.remove(&id);
    }
}
