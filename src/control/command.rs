mod log;
mod ps;
mod run;
mod stop;
mod stop_server;

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
    StopServer(stop_server::StopServerSubcommand),
}

macro_rules! dispatch_command {
    ($c_var:ident, $s_var:ident => $handler:expr) => {
        match $c_var {
            $crate::control::command::Command::Run($s_var) => $handler,
            $crate::control::command::Command::Stop($s_var) => $handler,
            $crate::control::command::Command::Log($s_var) => $handler,
            $crate::control::command::Command::Ps($s_var) => $handler,
            $crate::control::command::Command::StopServer($s_var) => $handler,
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
    async fn handle_response(&mut self, resp: &str) -> Result<()>;
}

impl Command {
    pub(super) async fn run<C: IpcChannel>(
        self,
        ctx: &ControlContext,
        channel: &mut C,
    ) -> Result<()> {
        dispatch_command!(self, subcommand => subcommand.run(ctx, channel).await?);

        Ok(())
    }
}

impl CommandClient for Command {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        dispatch_command!(self, subcommand => subcommand.handler())
    }
}
