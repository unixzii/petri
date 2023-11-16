use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{self as tokio_io, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use tokio::task;

use super::{command, env, Context};

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

#[derive(Serialize, Deserialize)]
pub enum OwnedIpcMessagePacket<T> {
    Output(String),
    Response(T),
}

impl<T> OwnedIpcMessagePacket<T> {
    pub fn to_output(&self) -> Option<&str> {
        match self {
            OwnedIpcMessagePacket::Output(value) => Some(value),
            _ => None,
        }
    }
}

impl OwnedIpcMessagePacket<serde_json::Value> {
    pub fn into_response<U>(self) -> Option<serde_json::Result<U>>
    where
        U: DeserializeOwned + 'static,
    {
        match self {
            OwnedIpcMessagePacket::Response(value) => Some(serde_json::from_value(value)),
            _ => None,
        }
    }
}

struct Inner {
    id_seed: AtomicU64,
    pairs: RwLock<HashMap<u64, ControlPair>>,

    ctx: Arc<Context>,
}

struct ControlPair;

pub(super) struct IpcChannel {
    stream: UnixStream,
}

impl IpcChannel {
    pub fn stream_mut(&mut self) -> &mut UnixStream {
        &mut self.stream
    }

    pub async fn write_response<T>(&mut self, resp: T) -> tokio_io::Result<()>
    where
        T: Serialize + Send + Sync + 'static,
    {
        let msg = OwnedIpcMessagePacket::Response(resp);
        self.write_packet(&msg).await
    }

    pub async fn write_output(&mut self, s: &str) -> tokio_io::Result<()> {
        let msg = OwnedIpcMessagePacket::<()>::Output(s.to_owned());
        self.write_packet(&msg).await
    }

    async fn write_packet<'a, T>(&mut self, pkt: &OwnedIpcMessagePacket<T>) -> tokio_io::Result<()>
    where
        T: Serialize + Send + Sync + 'static,
    {
        let mut json_string = match serde_json::to_string(pkt) {
            Ok(s) => s,
            Err(err) => return Err(tokio_io::Error::new(tokio_io::ErrorKind::Other, err)),
        };
        json_string.push('\n');

        self.stream.write_all(json_string.as_bytes()).await?;
        self.stream.flush().await?;
        Ok(())
    }
}

pub(super) async fn serve_cli(ctx: Arc<Context>) -> Result<()> {
    let sock_path = env::socket_path()?;
    let listener = UnixListener::bind(sock_path)?;

    let inner = Arc::new(Inner {
        id_seed: Default::default(),
        pairs: Default::default(),
        ctx,
    });

    // Make sure the socket file is removed when the future is done
    // or early cancelled.
    struct DropGuard<'a>(&'a UnixListener);

    impl<'a> Drop for DropGuard<'a> {
        #[rustfmt::skip]
        fn drop(&mut self) {
            let sock_addr = self.0
                .local_addr()
                .expect("could not get local address");
            let sock_file_path = sock_addr
                .as_pathname()
                .expect("the socket should have a path name");
            if let Err(err) = fs::remove_file(sock_file_path) {
                error!("failed to cleanup the socket: {err:?}");
            }
        }
    }

    let _drop_guard = DropGuard(&listener);
    inner.accept_loop(&listener).await;

    Ok(())
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
                let stream = reader
                    .into_inner()
                    .reunite(write_half)
                    .expect("should reunite into stream");
                if let Err(err) = inner.run_command(&line, stream).await {
                    error!("failed to run command: {:?}", err);
                }
            } else {
                error!("failed to read from the stream");
            }

            inner.pairs.write().await.remove(&id);
        });
    }

    async fn run_command(self: &Arc<Self>, payload: &str, stream: UnixStream) -> Result<()> {
        let mut ipc_channel = IpcChannel { stream };
        let request: OwnedIpcRequestPacket = serde_json::from_str(payload)?;
        let cmd = request.cmd;

        let client_env = ClientEnv {
            cwd: request.cwd,
            env: request.env,
        };

        CLIENT_ENV
            .scope(client_env, async move {
                cmd.run(&self.ctx, &mut ipc_channel).await
            })
            .await
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
