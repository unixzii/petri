use anyhow::Result;
use clap::Args;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use crate::control::Context as ControlContext;
use crate::proc_mgr::StartInfo;

#[derive(Args, Debug)]
pub struct RunSubcommand {
    #[arg(required = true, last = true)]
    cmd_line: Vec<String>,
}

impl RunSubcommand {
    pub async fn run(self, ctx: &ControlContext, stream: &mut UnixStream) -> Result<()> {
        let (program, args) = {
            let mut cmd_line = self.cmd_line;
            let args = cmd_line.split_off(1);
            (cmd_line, if args.is_empty() { None } else { Some(args) })
        };

        let Some(program) = program.into_iter().next() else {
            stream.write_all(b"program must be specified").await?;
            return Err(anyhow!("no program is specified").context("run"));
        };

        // TODO: get cwd from the calling control process.
        let cwd = std::env::current_dir()?.to_str().unwrap_or("/").to_string();

        let pid = match ctx
            .proc_mgr_handle
            .add_process(StartInfo { program, args, cwd })
            .await
        {
            Ok(id) => id,
            Err(err) => {
                stream
                    .write_all(b"failed to start the process (maybe it exited too early)")
                    .await?;
                return Err(err.context("run"));
            }
        };

        stream
            .write_all(format!("process started (pid: {pid})").as_bytes())
            .await?;

        Ok(())
    }
}
