use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::control::{Context as ControlContext, IpcChannel};

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct StopSubcommand {
    /// Stop the process with the given pid.
    #[arg(short, long, required = true)]
    pid: u32,
}

impl StopSubcommand {
    pub(super) async fn run<C: IpcChannel>(
        self,
        ctx: &ControlContext,
        channel: &mut C,
    ) -> Result<()> {
        match ctx.proc_mgr_handle.stop_process(self.pid).await {
            Ok(exit_code) => {
                channel
                    .write_line(&format!("process stopped with exit code {exit_code}"))
                    .await?;
            }
            Err(err) => {
                channel
                    .write_line("failed to stop the process (is it running?)")
                    .await?;
                return Err(err.context("stop"));
            }
        }

        Ok(())
    }
}
