mod job;
mod log;
mod ps;
mod run;
mod stop;
mod stop_server;

use anyhow::Result;
use async_trait::async_trait;
use clap::Parser;
use serde::{Deserialize, Serialize};

use super::cli::{IpcChannel, OwnedIpcMessagePacket};
use super::Context as ControlContext;

const AFTER_HELP: &str = color_print::cstr!(
    "Run '<bold>petri help <<command>></bold>' for more information on a specific command."
);

#[derive(Parser, Serialize, Deserialize, Debug)]
#[command(name = "petri")]
#[command(about = "A minimalist process manager")]
#[command(after_help = AFTER_HELP)]
pub enum Command {
    /// Run an arbitrary command.
    Run(run::RunSubcommand),
    /// Stop a currently running process.
    Stop(stop::StopSubcommand),
    /// Stream logs of a process.
    Log(log::LogSubcommand),
    /// List processes.
    Ps(ps::PsSubcommand),
    /// Manage jobs.
    #[command(subcommand)]
    Job(job::JobSubcommand),
    /// Request the server to stop.
    StopServer(stop_server::StopServerSubcommand),
}

macro_rules! dispatch_command {
    ($c_var:ident, $s_var:ident => $handler:expr) => {
        match $c_var {
            Command::Run($s_var) => $handler,
            Command::Stop($s_var) => $handler,
            Command::Log($s_var) => $handler,
            Command::Ps($s_var) => $handler,
            Command::Job(job_subcommand) => match job_subcommand {
                job::JobSubcommand::Ls($s_var) => $handler,
            },
            Command::StopServer($s_var) => $handler,
        }
    };
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
    async fn handle_response(
        &mut self,
        resp: OwnedIpcMessagePacket<serde_json::Value>,
    ) -> Result<()>;
}

impl Command {
    pub(super) async fn run(self, ctx: &ControlContext, channel: &mut IpcChannel) -> Result<()> {
        dispatch_command!(self, subcommand => subcommand.run(ctx, channel).await?);

        Ok(())
    }
}

impl CommandClient for Command {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        dispatch_command!(self, subcommand => subcommand.handler())
    }
}
