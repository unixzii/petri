use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, RwLock};
use tokio::task;

use super::{command, env, Context, IpcChannel, IpcChannelFlavor};

#[derive(Serialize, Deserialize)]
pub struct IpcRequestPacket {
    pub cmd: command::Command,
}

pub(super) struct CliControl {
    shutdown_signal: oneshot::Sender<()>,
    shutdown_result: oneshot::Receiver<()>,
}

struct Inner {
    id_seed: AtomicU64,
    pairs: RwLock<HashMap<u64, ControlPair>>,

    ctx: Arc<Context>,
}

struct ControlPair;

impl CliControl {
    pub fn new(ctx: Arc<Context>) -> Result<Self> {
        let sock_path = env::socket_path()?;
        let listener = UnixListener::bind(sock_path)?;

        let (shutdown_signal_tx, shutdown_signal_rx) = oneshot::channel();
        let (shutdown_result_tx, shutdown_result_rx) = oneshot::channel();

        let inner = Arc::new(Inner {
            id_seed: Default::default(),
            pairs: Default::default(),
            ctx,
        });

        let inner_clone = Arc::clone(&inner);
        task::spawn(async move {
            tokio::select! {
                _ = shutdown_signal_rx => { },
                _ = inner_clone.accept_loop(&listener) => {
                    unreachable!("`accept_loop` should not return")
                }
            }

            // Close and cleanup the socket.
            // TODO: force closing all active connections.
            let sock_addr = listener.local_addr().expect("could not get local address");
            fs::remove_file(
                sock_addr
                    .as_pathname()
                    .expect("the socket should have a path name"),
            )
            .expect("failed to cleanup the socket");

            _ = shutdown_result_tx.send(());
        });

        Ok(Self {
            shutdown_signal: shutdown_signal_tx,
            shutdown_result: shutdown_result_rx,
        })
    }

    pub async fn shutdown(self) {
        self.shutdown_signal
            .send(())
            .expect("background task exited too early");

        self.shutdown_result
            .await
            .expect("expected shutdown result");
    }
}

impl Inner {
    async fn accept_loop(self: &Arc<Self>, listener: &UnixListener) {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("new connection from: {:?}", addr);
                    self.serve_connection(stream).await;
                }
                Err(e) => {
                    // TODO: is it ok to continue accepting new connections?
                    println!("failed to accept new connection: {}", e);
                    continue;
                }
            }
        }
    }

    async fn serve_connection(self: &Arc<Self>, stream: UnixStream) {
        let id = self.id_seed.fetch_add(1, AtomicOrdering::Relaxed);

        let pair = ControlPair;
        self.pairs.write().await.insert(id, pair);

        let inner = Arc::clone(self);
        task::spawn(async move {
            let (read_half, write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);

            let mut line = String::new();
            if reader.read_line(&mut line).await.is_ok() {
                let stream = reader
                    .into_inner()
                    .reunite(write_half)
                    .expect("should reunite into stream");
                if let Err(err) = inner.run_command(&line, stream).await {
                    println!("failed to run command: {:?}", err);
                }
            } else {
                println!("failed to read from the stream");
            }

            inner.pairs.write().await.remove(&id);
        });
    }

    async fn run_command(self: &Arc<Self>, payload: &str, mut stream: UnixStream) -> Result<()> {
        let request: IpcRequestPacket = serde_json::from_str(payload)?;
        request.cmd.run(&self.ctx, &mut stream).await
    }
}

impl IpcChannel for UnixStream {
    fn flavor(&self) -> IpcChannelFlavor {
        IpcChannelFlavor::Cli
    }
}
