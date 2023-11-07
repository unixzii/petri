#[macro_use(anyhow)]
extern crate anyhow;

#[macro_use]
extern crate log;

pub mod cli;
pub mod command;
pub mod env;

use std::sync::Arc;

use anyhow::Result;
use petri_core::process_mgr::Handle as ProcessManagerHandle;
use tokio::sync::watch;

use cli::CliControl;
pub use command::Command;

pub struct Control {
    cli: CliControl,
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
