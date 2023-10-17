use anyhow::Result;
use clap::Args;
use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::mpsc;

use crate::control::Context as ControlContext;

#[derive(Args, Debug)]
pub struct LogSubcommand {
    /// Stream logs of a currently running process with the given pid.
    #[arg(short, long, required = true)]
    pid: u32,
}

impl LogSubcommand {
    pub async fn run(self, ctx: &ControlContext, write_half: &mut OwnedWriteHalf) -> Result<()> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let Some(cancel_token) = ctx
            .proc_mgr_handle
            .attach_output_channel(self.pid, tx)
            .await
        else {
            write_half
                .write_all(b"failed to stream logs from the process (is it running?)")
                .await?;
            return Err(anyhow!("failed to stream logs").context("log"));
        };

        let mut peer_closed = false;
        while let Some(contents) = rx.recv().await {
            if write_half.write_all(&contents).await.is_err() {
                peer_closed = true;
                break;
            }
        }

        drop(cancel_token);

        if peer_closed {
            println!(
                "ended streaming logs from process {} because the peer is closed",
                self.pid
            );
        } else {
            println!(
                "ended streaming logs from process {} because it exited",
                self.pid
            );
        }

        Ok(())
    }
}
