#[macro_use]
extern crate anyhow;

mod daemon;
mod proc_mgr;
mod util;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    daemon::run_daemon().await;
}
