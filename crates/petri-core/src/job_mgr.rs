use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::sync::{Arc, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use chrono::{DateTime, Local};
use indexmap::IndexMap;
use petri_utils::subscriber_list::CancellationToken;
use sha1::digest::OutputSizeUser;
use sha1::{Digest, Sha1};
use tokio::sync::RwLock;
use tokio::task;

use crate::process::StartInfo;
use crate::process_mgr::{self, Handle as ProcessManagerHandle};

#[derive(Clone, Debug)]
pub struct JobDescription {
    pub start_info: StartInfo,
    pub auto_restart: bool,
}

#[derive(Clone, Debug)]
pub struct Job {
    id: String,
    desc: JobDescription,
    created_at: DateTime<Local>,
    pid: Option<u32>,
    last_exit_code: Option<i32>,
}

pub struct JobManager {
    handle: Handle,
}

#[derive(Clone)]
pub struct Handle {
    inner: Arc<Inner>,
}

struct ProcessManagerEventHandler {
    weak_ptr: Weak<Inner>,
}

struct Inner {
    proc_mgr_handle: ProcessManagerHandle,
    jobs: RwLock<IndexMap<String, Job>>,
    pid_index: RwLock<HashMap<u32, String>>,
    _cancellation_token: CancellationToken<Box<dyn process_mgr::EventHandler>>,
}

impl JobDescription {
    fn digest(&self, seed: u64) -> String {
        let mut hasher = Sha1::new();

        hasher.update(seed.to_be_bytes());
        hasher.update(self.start_info.program.as_bytes());
        hasher.update(b"(");
        if let Some(args) = &self.start_info.args {
            for arg in args {
                hasher.update(arg.as_bytes());
                hasher.update(b",");
            }
        }
        hasher.update(b")");
        hasher.update(self.start_info.cwd.as_bytes());
        hasher.update(b"{");
        for env in self.start_info.env.iter() {
            hasher.update(env.0.as_bytes());
            hasher.update(b":");
            hasher.update(env.1.as_bytes());
            hasher.update(b",");
        }
        hasher.update(b"}");
        if let Some(log_path) = &self.start_info.log_path {
            hasher.update(log_path.as_os_str().as_bytes());
        }
        hasher.update(&[self.auto_restart as u8]);

        let digest = hasher.finalize();
        digest.iter().fold(
            String::with_capacity(<Sha1 as OutputSizeUser>::output_size() * 2),
            |mut hex, octet| {
                hex.push_str(&format!("{octet:02x}"));
                hex
            },
        )
    }
}

impl Job {
    #[inline]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[inline]
    pub fn description(&self) -> &JobDescription {
        &self.desc
    }

    #[inline]
    pub fn created_at(&self) -> &DateTime<Local> {
        &self.created_at
    }

    #[inline]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    #[inline]
    pub fn last_exit_code(&self) -> Option<i32> {
        self.last_exit_code
    }
}

impl JobManager {
    pub fn new(proc_mgr_handle: ProcessManagerHandle) -> Self {
        let handle = Handle {
            inner: Arc::new_cyclic(|me| {
                let event_handler = ProcessManagerEventHandler {
                    weak_ptr: me.clone(),
                };
                let token = proc_mgr_handle.add_event_handler(event_handler);

                Inner {
                    proc_mgr_handle,
                    jobs: Default::default(),
                    pid_index: Default::default(),
                    _cancellation_token: token,
                }
            }),
        };

        Self { handle }
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }
}

impl Handle {
    pub async fn jobs(&self) -> Vec<Job> {
        let jobs = self.inner.jobs.read().await;
        jobs.values().cloned().collect()
    }

    pub async fn add_job(&self, job: JobDescription) -> Result<String> {
        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current system date is invalid")
            .as_millis() as u64;
        let digest = job.digest(now_ts);

        let mut jobs = self.inner.jobs.write().await;
        if jobs.contains_key(&digest) {
            return Err(anyhow!("job id has been already used"));
        }
        jobs.insert(
            digest.clone(),
            Job {
                id: digest.clone(),
                desc: job,
                created_at: Local::now(),
                pid: None,
                last_exit_code: None,
            },
        );

        Ok(digest)
    }

    pub async fn start_job(&self, jid: &str) -> Result<u32> {
        let mut jobs = self.inner.jobs.write().await;
        let mut pid_index = self.inner.pid_index.write().await;

        let Some(job) = jobs.get_mut(jid) else {
            return Err(anyhow!("job with id `{jid}` is not found"));
        };

        if job.pid.is_some() {
            return Err(anyhow!("job is already started"));
        }

        let pid = self
            .inner
            .proc_mgr_handle
            .add_process(&job.desc.start_info)
            .await?;
        job.pid = Some(pid);
        pid_index.insert(pid, jid.to_owned());

        Ok(pid)
    }

    async fn handle_process_exit(&self, pid: u32, exit_code: i32) {
        let mut jobs = self.inner.jobs.write().await;
        let mut pid_index = self.inner.pid_index.write().await;

        let Some(jid) = pid_index.get(&pid) else {
            debug!("no matching job with pid: {pid}");
            return;
        };

        let job = jobs.get_mut(jid).expect("internal state is inconsistent");
        job.pid = None;
        job.last_exit_code = Some(exit_code);

        pid_index.remove(&pid);
    }
}

impl process_mgr::EventHandler for ProcessManagerEventHandler {
    fn handle_process_exit(&self, pid: u32, exit_code: i32) {
        let Some(strong_ptr) = self.weak_ptr.upgrade() else {
            return;
        };
        task::spawn(async move {
            (Handle { inner: strong_ptr })
                .handle_process_exit(pid, exit_code)
                .await
        });
    }
}
