#[macro_use]
extern crate log;

mod client;
mod logging;
mod server;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<_> = std::env::args().collect();

    if args.len() == 2 && args[1] == "--server" {
        server::run_server().await;
        return;
    }

    client::run_client(args).await;
}
