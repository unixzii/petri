mod log;
mod run;
mod stop;

use anyhow::Result;
use clap::{ColorChoice, Parser};
use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedWriteHalf;

use super::{Context as ControlContext, Message as ControlMessage};

#[derive(Parser, Debug)]
#[command(name = "petri")]
#[command(bin_name = "petri")]
#[command(color = ColorChoice::Always)]
enum Command {
    /// Run an arbitrary command.
    Run(run::RunSubcommand),
    /// Stop a currently running process.
    Stop(stop::StopSubcommand),
    /// Stream logs of a process.
    Log(log::LogSubcommand),
    /// Request the server to stop.
    StopServer,
}

pub async fn run_command(
    args: &[String],
    ctx: &ControlContext,
    write_half: &mut OwnedWriteHalf,
) -> Result<()> {
    let command = match Command::try_parse_from(args) {
        Ok(command) => command,
        Err(err) => {
            let help_string = err.render().ansi().to_string();
            write_half.write_all(help_string.as_bytes()).await?;
            return Err(err.into());
        }
    };

    match command {
        Command::Run(subcommand) => subcommand.run(ctx, write_half).await?,
        Command::Stop(subcommand) => subcommand.run(ctx, write_half).await?,
        Command::Log(subcommand) => subcommand.run(ctx, write_half).await?,
        Command::StopServer => {
            _ = ctx.message_tx.send(ControlMessage::RequestShutdown).await;
            write_half
                .write_all(b"requested the server to shutdown")
                .await?;
        }
    }

    Ok(())
}
