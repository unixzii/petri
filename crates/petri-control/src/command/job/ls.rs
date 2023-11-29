use anyhow::Result;
use async_trait::async_trait;
use chrono::DateTime;
use clap::Args;
use petri_utils::console_table;
use serde::{Deserialize, Serialize};

use crate::cli::{IpcChannel, OwnedIpcMessagePacket};
use crate::command::{CommandClient, ResponseHandler};
use crate::Context as ControlContext;

#[derive(Serialize, Deserialize, Debug)]
struct ListResponse {
    jobs: Vec<Job>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Job {
    jid: String,
    pid: Option<u32>,
    cmd: String,
    created_at_ts: (i64, u32),
}

#[derive(Args, Serialize, Deserialize, Debug)]
pub struct ListSubcommand;

impl ListSubcommand {
    pub(in crate::command) async fn run(
        self,
        ctx: &ControlContext,
        channel: &mut IpcChannel,
    ) -> Result<()> {
        let real_jobs = ctx.job_mgr_handle.jobs().await;

        let mut jobs = vec![];
        for job in real_jobs {
            let created_at = job.created_at();
            jobs.push(Job {
                jid: job.id().to_owned(),
                pid: job.pid(),
                cmd: job.description().start_info.cmd(),
                created_at_ts: (created_at.timestamp(), created_at.timestamp_subsec_nanos()),
            });
        }

        let resp = ListResponse { jobs };
        channel.write_response(resp).await?;
        Ok(())
    }
}

impl CommandClient for ListSubcommand {
    fn handler(&self) -> Option<Box<dyn ResponseHandler>> {
        Some(Box::new(ListResponseHandler))
    }
}

struct ListResponseHandler;

#[async_trait]
impl ResponseHandler for ListResponseHandler {
    async fn handle_response(
        &mut self,
        resp: OwnedIpcMessagePacket<serde_json::Value>,
    ) -> Result<()> {
        let resp: ListResponse = resp.into_response().expect("expected a response")?;
        let mut jobs = resp.jobs;

        // Sort the job list by their created time.
        jobs.sort_by_key(|job| DateTime::from_timestamp(job.created_at_ts.0, job.created_at_ts.1));

        let jid_column = console_table::ColumnOptions::new("JID");
        let pid_column = console_table::ColumnOptions::new("PID")
            .alignment(console_table::Alignment::Right)
            .spacing(2);
        let cmd_column = console_table::ColumnOptions::new("CMD");

        let table = console_table::Builder::new()
            .with_new_columns((jid_column, pid_column, cmd_column), |insert| {
                for job in jobs {
                    let pid_string = job.pid.map(|pid| pid.to_string()).unwrap_or_default();
                    insert((job.jid, pid_string, job.cmd));
                }
            })
            .build();

        println!("{table}");

        Ok(())
    }
}
