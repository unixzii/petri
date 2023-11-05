use anyhow::Result;
use async_trait::async_trait;
use clap::Args;
use serde::{Deserialize, Serialize};

use super::{CommandClient, IpcChannel, OwnedIpcMessagePacket, ResponseHandler};
use crate::control::Context as ControlContext;

#[derive(Serialize, Deserialize, Debug)]
pub struct PsResponse {
    processes: Vec<Process>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Process {
    pid: u32,
    cmd: String,
}

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct PsSubcommand;

impl PsSubcommand {
    pub(super) async fn run(self, ctx: &ControlContext, channel: &mut IpcChannel) -> Result<()> {
        let processes = ctx.proc_mgr_handle.processes().await;
        let resp = PsResponse {
            processes: processes
                .into_iter()
                .map(|proc| Process {
                    pid: proc.id(),
                    cmd: proc.cmd().to_owned(),
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
        let pid_column_width = resp
            .processes
            .iter()
            .map(|proc| proc.pid)
            .max()
            .unwrap_or_default()
            .to_string()
            .len()
            .max(3);
        println!("{:>pid_width$}   CMD", "PID", pid_width = pid_column_width);
        for proc in resp.processes {
            println!(
                "{:>pid_width$}   {}",
                proc.pid,
                proc.cmd,
                pid_width = pid_column_width
            );
        }
        Ok(())
    }
}
