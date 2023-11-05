use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};

use super::{CommandClient, IpcChannel, ResponseHandler};
use crate::control::Context as ControlContext;

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct StopSubcommand {
    /// Stop the process with the given pid.
    #[arg(short, long, required = true)]
    pid: u32,
}

impl StopSubcommand {
    pub(super) async fn run(self, ctx: &ControlContext, channel: &mut IpcChannel) -> Result<()> {
        match ctx.proc_mgr_handle.stop_process(self.pid).await {
            Ok(exit_code) => {
                channel
                    .write_output(&format!("process stopped with exit code {exit_code}\n"))
                    .await?;
            }
            Err(err) => {
                channel
                    .write_output("failed to stop the process (is it running?)\n")
                    .await?;
                return Err(err.context("stop"));
            }
        }

        Ok(())
    }
}

impl CommandClient for StopSubcommand {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        None
    }
}
