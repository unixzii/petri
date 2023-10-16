mod command;
pub mod env;

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, watch};
use tokio::sync::{Mutex, RwLock};
use tokio::task;

use crate::proc_mgr::Handle as ProcessManagerHandle;

pub struct Control {
    inner: Arc<Inner>,
}

pub(crate) struct Context {
    pub proc_mgr_handle: ProcessManagerHandle,
}

struct Inner {
    id_seed: AtomicU64,
    pairs: RwLock<HashMap<u64, ControlPair>>,

    ctx: Context,

    shutdown_signal: watch::Sender<bool>,
    shutdown_result: Mutex<Option<oneshot::Receiver<()>>>,
}

struct ControlPair;

impl Control {
    pub fn new(proc_mgr_handle: ProcessManagerHandle) -> Result<Control> {
        let sock_path = env::socket_path()?;
        let listener = UnixListener::bind(sock_path)?;

        let (shutdown_signal_tx, mut shutdown_signal_rx) = watch::channel(false);
        let (shutdown_result_tx, shutdown_result_rx) = oneshot::channel();

        let inner = Arc::new(Inner {
            id_seed: Default::default(),
            pairs: Default::default(),
            ctx: Context { proc_mgr_handle },
            shutdown_signal: shutdown_signal_tx,
            shutdown_result: Mutex::new(Some(shutdown_result_rx)),
        });

        let inner_clone = Arc::clone(&inner);
        task::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_signal_rx.changed() => {
                        if *shutdown_signal_rx.borrow_and_update() {
                            break;
                        }
                    },
                    _ = inner_clone.accept_loop(&listener) => {
                        unreachable!("`accept_loop` should not return")
                    }
                }
            }

            // Close and cleanup the socket.
            let sock_addr = listener.local_addr().expect("could not get local address");
            fs::remove_file(
                sock_addr
                    .as_pathname()
                    .expect("the socket should have a path name"),
            )
            .expect("failed to cleanup the socket");

            _ = shutdown_result_tx.send(());
        });

        Ok(Self { inner })
    }

    pub async fn shutdown(&self) {
        let mut shutdown_result = self.inner.shutdown_result.lock().await;
        let Some(shutdown_result) = shutdown_result.take() else {
            return;
        };

        _ = self.inner.shutdown_signal.send(true);
        shutdown_result.await.expect("expected shutdown result");
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
                if let Err(err) = inner.run_command(&line, write_half).await {
                    println!("failed to run command: {:?}", err);
                }
            } else {
                println!("failed to read from the stream");
            }

            inner.pairs.write().await.remove(&id);
        });
    }

    async fn run_command(
        self: &Arc<Self>,
        payload: &str,
        mut write_half: OwnedWriteHalf,
    ) -> Result<()> {
        let args: Vec<String> = serde_json::from_str(payload)?;
        command::run_command(&args, &self.ctx, &mut write_half).await
    }
}
