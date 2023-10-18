use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use crate::control::Context as ControlContext;

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct LogSubcommand {
    /// Stream logs of a currently running process with the given pid.
    #[arg(short, long, required = true)]
    pid: u32,
}

impl LogSubcommand {
    pub async fn run<S: AsyncRead + AsyncWrite + Unpin>(
        self,
        ctx: &ControlContext,
        stream: &mut S,
    ) -> Result<()> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let Some(cancel_token) = ctx
            .proc_mgr_handle
            .attach_output_channel(self.pid, tx)
            .await
        else {
            stream
                .write_all(b"failed to stream logs from the process (is it running?)")
                .await?;
            return Err(anyhow!("failed to stream logs").context("log"));
        };

        let mut peer_closed = false;
        loop {
            // We don't expect to read any bytes here, so we only use a small
            // buffer to check if the remote peer is closed.
            let mut buf = [0; 1];
            let Some(contents) = tokio::select! {
                contents = rx.recv() => { contents },
                read_res = stream.read(&mut buf) => {
                    if read_res.unwrap_or(0) == 0 {
                        peer_closed = true;
                        break;
                    }
                    println!("unexpected byte received: {}", buf[0]);
                    continue;
                }
            } else {
                break;
            };

            if stream.write_all(&contents).await.is_err() {
                peer_closed = true;
                break;
            }
            if stream.flush().await.is_err() {
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
