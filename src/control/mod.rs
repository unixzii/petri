pub mod cli;
pub mod command;
pub mod env;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::watch;

use crate::proc_mgr::Handle as ProcessManagerHandle;
use cli::CliControl;

pub struct Control {
    cli: CliControl,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IpcChannelFlavor {
    CliStdout,
    CliJson,
}

#[async_trait]
trait IpcChannel: AsyncRead + AsyncWrite + Send + Unpin {
    fn flavor(&self) -> IpcChannelFlavor;

    async fn write_response<T>(&mut self, msg: T) -> tokio::io::Result<()>
    where
        T: Serialize + Send,
    {
        if self.flavor() != IpcChannelFlavor::CliJson {
            return Ok(());
        }
        let json_string = match serde_json::to_string(&msg) {
            Ok(s) => s,
            Err(err) => return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, err)),
        };
        self.write_all(json_string.as_bytes()).await?;
        Ok(())
    }

    async fn write_line(&mut self, s: &str) -> tokio::io::Result<()> {
        if self.flavor() != IpcChannelFlavor::CliStdout {
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
