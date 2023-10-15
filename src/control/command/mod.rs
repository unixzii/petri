use anyhow::Result;
use clap::{ColorChoice, Parser};
use tokio::io::AsyncWriteExt;
use tokio::net::unix::OwnedWriteHalf;

#[derive(Parser, Debug)]
#[command(name = "petri")]
#[command(bin_name = "petri")]
#[command(color = ColorChoice::Always)]
enum Command {
    /// Request the server to stop.
    StopServer,
}

pub async fn run_command(args: &[String], write_half: &mut OwnedWriteHalf) -> Result<()> {
    let command = match Command::try_parse_from(args) {
        Ok(command) => command,
        Err(err) => {
            let help_string = err.render().ansi().to_string();
            write_half.write_all(help_string.as_bytes()).await?;
            return Err(err.into());
        }
    };

    let echo_string = format!("{:?}", command);
    write_half.write_all(echo_string.as_bytes()).await?;

    Ok(())
}
