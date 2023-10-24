mod log;
mod ps;
mod run;
mod stop;

use anyhow::Result;
use async_trait::async_trait;
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
    /// List processes.
    Ps(ps::PsSubcommand),
    /// Request the server to stop.
    StopServer,
}

/// Trait that specifies how the control client handles a command.
pub trait CommandClient {
    /// Returns an optional handler for JSON-format response.
    ///
    /// If the implementation returns `None`, then the command will
    /// run in stream mode, which directly writes the contents server
    /// sends to stdout.
    fn handler(&self) -> Option<Box<dyn ResponseHandler>>;
}

#[async_trait]
pub trait ResponseHandler {
    async fn handle_response(&mut self, resp: &str) -> Result<()>;
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
            Command::Ps(subcommand) => subcommand.run(ctx, channel).await?,
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

impl CommandClient for Command {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        match self {
            Command::Run(subcommand) => subcommand.handler(),
            Command::Stop(subcommand) => subcommand.handler(),
            Command::Log(subcommand) => subcommand.handler(),
            Command::Ps(subcommand) => subcommand.handler(),
            Command::StopServer => None,
        }
    }
}
