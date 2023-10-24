use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};

use super::{CommandClient, ResponseHandler};
use crate::control::{Context as ControlContext, IpcChannel};

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct StopServerSubcommand;

impl StopServerSubcommand {
    pub(super) async fn run<C: IpcChannel>(
        self,
        ctx: &ControlContext,
        channel: &mut C,
    ) -> Result<()> {
        _ = ctx.shutdown_request.send(true);
        channel
            .write_line("requested the server to shutdown")
            .await?;

        Ok(())
    }
}

impl CommandClient for StopServerSubcommand {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        None
    }
}
