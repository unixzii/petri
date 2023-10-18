mod log;
mod run;
mod stop;

use anyhow::Result;
use clap::{ColorChoice, Parser};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;

use super::Context as ControlContext;

#[derive(Parser, Serialize, Deserialize, Debug)]
#[command(name = "petri")]
#[command(bin_name = "petri")]
#[command(color = ColorChoice::Always)]
pub enum Command {
    /// Run an arbitrary command.
    Run(run::RunSubcommand),
    /// Stop a currently running process.
    Stop(stop::StopSubcommand),
    /// Stream logs of a process.
    Log(log::LogSubcommand),
    /// Request the server to stop.
    StopServer,
}

impl Command {
    pub async fn run<S: AsyncRead + AsyncWrite + Unpin>(
        self,
        ctx: &ControlContext,
        stream: &mut S,
    ) -> Result<()> {
        match self {
            Command::Run(subcommand) => subcommand.run(ctx, stream).await?,
            Command::Stop(subcommand) => subcommand.run(ctx, stream).await?,
            Command::Log(subcommand) => subcommand.run(ctx, stream).await?,
            Command::StopServer => {
                _ = ctx.shutdown_request.send(true);
                stream
                    .write_all(b"requested the server to shutdown")
                    .await?;
            }
        }

        Ok(())
    }
}
