use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};

use super::{CommandClient, IpcChannel, ResponseHandler};
use crate::Context as ControlContext;

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct StopServerSubcommand;

impl StopServerSubcommand {
    pub(super) async fn run(self, ctx: &ControlContext, channel: &mut IpcChannel) -> Result<()> {
        _ = ctx.shutdown_request.send(true);
        channel
            .write_output("requested the server to shutdown\n")
            .await?;

        Ok(())
    }
}

impl CommandClient for StopServerSubcommand {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        None
    }
}
