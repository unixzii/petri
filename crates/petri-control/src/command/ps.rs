use std::time::{Duration, Instant};

use anyhow::Result;
use async_trait::async_trait;
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
    pid: u32,
    cmd: String,
    uptime_secs: u64,
}

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct PsSubcommand;

impl PsSubcommand {
    pub(super) async fn run(self, ctx: &ControlContext, channel: &mut IpcChannel) -> Result<()> {
        let processes = ctx.proc_mgr_handle.processes().await;
        let now = Instant::now();
        let resp = PsResponse {
            processes: processes
                .into_iter()
                .map(|proc| Process {
                    pid: proc.id(),
                    cmd: proc.cmd().to_owned(),
                    uptime_secs: (now - proc.started_at()).as_secs(),
                })
                .collect(),
        };
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

        // Format all the fields into string and cache them, because we need to iterate
        // them multiple times to calculate the column width.
        let formatted_rows: Vec<_> = resp
            .processes
            .into_iter()
            .map(|proc| {
                let pid_string = proc.pid.to_string();
                let uptime = FormattedUptime::new(Duration::from_secs(proc.uptime_secs));
                let status_string = format!("Up {}", uptime);
                let cmd = proc.cmd;
                (pid_string, status_string, cmd)
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
            .map(|cols| cols.1.len())
            .max()
            .unwrap_or_default()
            .max(5);

        println!(
            "{:>pid_column_width$}  {:<status_column_width$}   CMD",
            "PID", "STATUS"
        );
        for row in formatted_rows {
            println!(
                "{:>pid_column_width$}  {:<status_column_width$}   {}",
                row.0, row.1, row.2,
            );
        }
        Ok(())
    }
}
