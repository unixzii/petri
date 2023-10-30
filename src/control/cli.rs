use std::collections::HashMap;
use std::fs;
use std::io;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::task::Poll;

use anyhow::Result;
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, RwLock};
use tokio::task;

use super::{command, env, Context, IpcChannel, IpcChannelFlavor};

#[derive(Serialize, Deserialize)]
pub struct OwnedIpcRequestPacket {
    pub cmd: command::Command,
    pub cwd: String,
    pub env: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct IpcRequestPacket<'c> {
    pub cmd: &'c command::Command,
    pub cwd: String,
    pub env: HashMap<String, String>,
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

#[pin_project]
struct UnixStreamWrapper {
    flavor: IpcChannelFlavor,
    #[pin]
    stream: UnixStream,
}

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
                    debug!("new connection from: {:?}", addr);
                    self.serve_connection(stream).await;
                }
                Err(e) => {
                    // TODO: is it ok to continue accepting new connections?
                    error!("failed to accept new connection: {}", e);
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
                let flavor = if line.starts_with('>') {
                    line.remove(0);
                    IpcChannelFlavor::CliJson
                } else {
                    IpcChannelFlavor::CliStdout
                };
                let stream = reader
                    .into_inner()
                    .reunite(write_half)
                    .expect("should reunite into stream");
                if let Err(err) = inner.run_command(&line, flavor, stream).await {
                    error!("failed to run command: {:?}", err);
                }
            } else {
                error!("failed to read from the stream");
            }

            inner.pairs.write().await.remove(&id);
        });
    }

    async fn run_command(
        self: &Arc<Self>,
        payload: &str,
        flavor: IpcChannelFlavor,
        stream: UnixStream,
    ) -> Result<()> {
        let mut stream_wrapper = UnixStreamWrapper { flavor, stream };
        let request: OwnedIpcRequestPacket = serde_json::from_str(payload)?;
        let cmd = request.cmd;

        let client_env = ClientEnv {
            cwd: request.cwd,
            env: request.env,
        };

        CLIENT_ENV
            .scope(client_env, async move {
                cmd.run(&self.ctx, &mut stream_wrapper).await
            })
            .await
    }
}

impl AsyncWrite for UnixStreamWrapper {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<StdResult<usize, io::Error>> {
        let this = self.project();
        this.stream.poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<StdResult<(), io::Error>> {
        let this = self.project();
        this.stream.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<StdResult<(), io::Error>> {
        let this = self.project();
        this.stream.poll_shutdown(cx)
    }
}

impl AsyncRead for UnixStreamWrapper {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        this.stream.poll_read(cx, buf)
    }
}

impl IpcChannel for UnixStreamWrapper {
    fn flavor(&self) -> IpcChannelFlavor {
        self.flavor
    }
}

#[derive(Debug, Clone)]
pub struct ClientEnv {
    cwd: String,
    env: HashMap<String, String>,
}

impl ClientEnv {
    #[inline(always)]
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    #[inline(always)]
    pub fn env(&self) -> &HashMap<String, String> {
        &self.env
    }
}

tokio::task_local! {
    pub static CLIENT_ENV: ClientEnv;
}
