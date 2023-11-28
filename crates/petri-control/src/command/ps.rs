use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use async_trait::async_trait;
use chrono::DateTime;
use clap::Args;
use petri_utils::time::FormattedUptime;
use serde::{Deserialize, Serialize};

use super::{CommandClient, IpcChannel, OwnedIpcMessagePacket, ResponseHandler};
use crate::Context as ControlContext;

#[derive(Serialize, Deserialize, Debug)]
pub struct PsResponse {
    processes: Vec<Process>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Process {
    jid: Option<String>,
    pid: Option<u32>,
    cmd: String,
    created_at_ts: (i64, u32),
    uptime_secs: u64,
    last_exit_code: Option<i32>,
}

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct PsSubcommand {
    /// Show all jobs (default shows just running)
    #[arg(short = 'a', long = "all")]
    show_all: bool,
}

impl PsSubcommand {
    pub(super) async fn run(self, ctx: &ControlContext, channel: &mut IpcChannel) -> Result<()> {
        let now = Instant::now();

        let jobs = ctx.job_mgr_handle.jobs().await;
        let running_processes = ctx.proc_mgr_handle.processes().await;

        let mut processes = vec![];
        let mut pid_index: HashMap<u32, usize> = HashMap::new();
        for proc in running_processes {
            let local_started_at = proc.local_started_at();
            pid_index.insert(proc.id(), processes.len());
            processes.push(Process {
                jid: None,
                pid: Some(proc.id()),
                cmd: proc.cmd().to_owned(),
                created_at_ts: (
                    local_started_at.timestamp(),
                    local_started_at.timestamp_subsec_nanos(),
                ),
                uptime_secs: (now - proc.started_at()).as_secs(),
                last_exit_code: None,
            });
        }

        // Mix jobs into the list, whose processes may have exited.
        for job in jobs {
            let jid = Some(job.id().to_owned());
            let created_at = job.created_at();
            if let Some(idx) = job.pid().and_then(|pid| pid_index.get(&pid)) {
                // Update the item to fill in `jid` and `created_at_ts`.
                let proc = &mut processes[*idx];
                proc.jid = jid;
                proc.created_at_ts = (created_at.timestamp(), created_at.timestamp_subsec_nanos());
            } else if self.show_all {
                // Also add the non-started jobs if `-a` flags is specified.
                processes.push(Process {
                    jid,
                    pid: None,
                    cmd: job.description().start_info.cmd(),
                    created_at_ts: (created_at.timestamp(), created_at.timestamp_subsec_nanos()),
                    uptime_secs: 0,
                    last_exit_code: job.last_exit_code(),
                })
            }
        }

        let resp = PsResponse { processes };
        channel.write_response(resp).await?;
        Ok(())
    }
}

impl CommandClient for PsSubcommand {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        Some(Box::new(PsResponseHandler))
    }
}

struct PsResponseHandler;

#[async_trait]
impl ResponseHandler for PsResponseHandler {
    async fn handle_response(
        &mut self,
        resp: OwnedIpcMessagePacket<serde_json::Value>,
    ) -> Result<()> {
        let resp: PsResponse = resp.into_response().expect("expected a response")?;
        let mut processes = resp.processes;

        // Sort the processes list by their created time.
        processes.sort_by_key(|proc| {
            DateTime::from_timestamp(proc.created_at_ts.0, proc.created_at_ts.1)
        });

        // Format all the fields into string and cache them, because we need to iterate
        // them multiple times to calculate the column width.
        let formatted_rows: Vec<_> = processes
            .into_iter()
            .map(|proc| {
                let pid_string = proc.pid.map(|pid| pid.to_string()).unwrap_or_default();
                let jid_string = proc.jid.map(|jid| jid[0..8].to_owned()).unwrap_or_default();
                let uptime = FormattedUptime::new(Duration::from_secs(proc.uptime_secs));
                let status_string = if proc.pid.is_some() {
                    format!("Up {uptime}")
                } else if let Some(last_exit_code) = proc.last_exit_code {
                    format!("Exited with code {last_exit_code}")
                } else {
                    "Not started".to_owned()
                };
                let cmd = proc.cmd;
                (pid_string, jid_string, status_string, cmd)
            })
            .collect();

        let pid_column_width = formatted_rows
            .iter()
            .map(|cols| cols.0.len())
            .max()
            .unwrap_or_default()
            .max(3);
        let status_column_width = formatted_rows
            .iter()
            .map(|cols| cols.2.len())
            .max()
            .unwrap_or_default()
            .max(5);

        println!(
            "{:>pid_column_width$} JID       {:<status_column_width$}   CMD",
            "PID", "STATUS"
        );
        for row in formatted_rows {
            println!(
                "{:>pid_column_width$} {:<8}  {:<status_column_width$}   {}",
                row.0, row.1, row.2, row.3,
            );
        }
        Ok(())
    }
}
