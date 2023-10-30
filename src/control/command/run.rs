use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use serde::{Deserialize, Serialize};

use super::{CommandClient, ResponseHandler};
use crate::control::cli::CLIENT_ENV;
use crate::control::{Context as ControlContext, IpcChannel};
use crate::proc_mgr::StartInfo;

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct RunSubcommand {
    /// Redirect stdout & stderr to log files in the given path.
    #[arg(short)]
    log_path: Option<PathBuf>,
    #[arg(required = true, last = true)]
    cmd_line: Vec<String>,
}

impl RunSubcommand {
    pub(super) async fn run<C: IpcChannel>(
        self,
        ctx: &ControlContext,
        channel: &mut C,
    ) -> Result<()> {
        let (program, args) = {
            let mut cmd_line = self.cmd_line;
            let args = cmd_line.split_off(1);
            (cmd_line, if args.is_empty() { None } else { Some(args) })
        };

        let Some(program) = program.into_iter().next() else {
            channel.write_line("program must be specified").await?;
            return Err(anyhow!("no program is specified").context("run"));
        };

        let (cwd, env_vars) = CLIENT_ENV
            .try_with(|env| (env.cwd().to_owned(), env.env().clone()))
            .expect("no `ClientEnv` set in the calling context");

        let start_info = StartInfo {
            program,
            args,
            cwd,
            env: env_vars,
            log_path: self.log_path,
        };

        let pid = match ctx.proc_mgr_handle.add_process(start_info).await {
            Ok(id) => id,
            Err(err) => {
                channel
                    .write_line("failed to start the process (maybe it exited too early)")
                    .await?;
                return Err(err.context("run"));
            }
        };

        channel
            .write_line(&format!("process started (pid: {pid})"))
            .await?;

        Ok(())
    }
}

impl CommandClient for RunSubcommand {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        None
    }
}
