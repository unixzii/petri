mod log;
mod run;
mod stop;

use anyhow::Result;
use clap::{ColorChoice, Parser};
use serde::{Deserialize, Serialize};

use super::{Context as ControlContext, IpcChannel};

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
    pub(super) async fn run<C: IpcChannel>(
        self,
        ctx: &ControlContext,
        channel: &mut C,
    ) -> Result<()> {
        match self {
            Command::Run(subcommand) => subcommand.run(ctx, channel).await?,
            Command::Stop(subcommand) => subcommand.run(ctx, channel).await?,
            Command::Log(subcommand) => subcommand.run(ctx, channel).await?,
            Command::StopServer => {
                _ = ctx.shutdown_request.send(true);
                channel
                    .write_line("requested the server to shutdown")
                    .await?;
            }
        }

        Ok(())
    }
}
