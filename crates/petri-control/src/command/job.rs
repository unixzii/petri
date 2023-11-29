mod ls;

use clap::Subcommand;
use serde::{Deserialize, Serialize};

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub enum JobSubcommand {
    /// List jobs
    Ls(ls::ListSubcommand),
}
