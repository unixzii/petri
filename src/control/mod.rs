pub mod cli;
pub mod command;
pub mod env;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, Result as TokioIoResult};
use tokio::sync::watch;

use crate::proc_mgr::Handle as ProcessManagerHandle;
use cli::CliControl;

pub struct Control {
    cli: CliControl,
}

#[derive(PartialEq, Eq, Debug)]
enum IpcChannelFlavor {
    Cli,
}

#[async_trait]
trait IpcChannel: AsyncRead + AsyncWrite + Send + Unpin {
    fn flavor(&self) -> IpcChannelFlavor;

    async fn write_line(&mut self, s: &str) -> TokioIoResult<()> {
        if self.flavor() != IpcChannelFlavor::Cli {
            return Ok(());
        }
        self.write_all(s.as_bytes()).await?;
        self.write_all(&[b'\n']).await?;
        Ok(())
    }
}

struct Context {
    pub proc_mgr_handle: ProcessManagerHandle,
    pub shutdown_request: watch::Sender<bool>,
}

impl Control {
    pub fn new(
        proc_mgr_handle: ProcessManagerHandle,
        shutdown_request: watch::Sender<bool>,
    ) -> Result<Self> {
        let ctx = Arc::new(Context {
            proc_mgr_handle,
            shutdown_request,
        });

        let cli = CliControl::new(ctx)?;

        Ok(Self { cli })
    }

    pub async fn shutdown(self) {
        self.cli.shutdown().await;
    }
}
