use anyhow::Result;
use clap::Args;
use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedWriteHalf;

use crate::control::Context as ControlContext;

#[derive(Args, Debug)]
pub struct StopSubcommand {
    /// Stop the process with the given pid.
    #[arg(short, long, required = true)]
    pid: u32,
}

impl StopSubcommand {
    pub async fn run(self, ctx: &ControlContext, write_half: &mut OwnedWriteHalf) -> Result<()> {
        match ctx.proc_mgr_handle.stop_process(self.pid).await {
            Ok(exit_code) => {
                write_half
                    .write_all(format!("process stopped with exit code {exit_code}").as_bytes())
                    .await?;
            }
            Err(err) => {
                write_half
                    .write_all(b"failed to stop the process (is it running?)")
                    .await?;
                return Err(err.context("stop"));
            }
        }

        Ok(())
    }
}
