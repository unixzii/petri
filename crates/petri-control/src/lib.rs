#[macro_use(anyhow)]
extern crate anyhow;

#[macro_use]
extern crate log;

pub mod cli;
pub mod command;
pub mod env;

use std::sync::Arc;

use anyhow::Result;
use petri_core::job_mgr::Handle as JobManagerHandle;
use petri_core::process_mgr::Handle as ProcessManagerHandle;
use tokio::sync::watch;

pub use command::Command;

pub struct Context {
    pub proc_mgr_handle: ProcessManagerHandle,
    pub job_mgr_handle: JobManagerHandle,
    pub shutdown_request: watch::Sender<bool>,
}

pub async fn run_control_server(ctx: Context) -> Result<()> {
    cli::serve_cli(Arc::new(ctx)).await
}
