use std::collections::HashMap;
use std::env;
use std::error::Error as StdError;
use std::ffi::OsStr;
use std::io::{self, ErrorKind as IoErrorKind, Write};
use std::os::unix::prelude::OsStrExt;
use std::process::{self, Stdio};

use anyhow::Error;
use clap::Parser;
use petri_control::cli::{IpcRequestPacket, OwnedIpcMessagePacket};
use petri_control::command::CommandClient;
use petri_control::env::socket_path;
use petri_control::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

enum ConnectError {
    ServerNotStarted,
    OtherError(Error),
}

impl<E> From<E> for ConnectError
where
    E: StdError + Send + Sync + 'static,
{
    fn from(value: E) -> Self {
        ConnectError::OtherError(Error::new(value))
    }
}

pub async fn run_client(args: Vec<String>) {
    // Collect execution environment of the client.
    let Some(cwd) = env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
    else {
        println!("current dir is invalid");
        return;
    };
    let env_vars: HashMap<_, _> = env::vars_os()
        .filter_map(|entry| {
            let Some(key) = entry.0.to_str() else {
                return None;
            };
            let Some(value) = entry.1.to_str() else {
                return None;
            };
            Some((key.to_string(), value.to_string()))
        })
        .collect();

    // Parse and serialize the command.
    let cmd = Command::parse_from(args);
    let mut cmd_string = serde_json::to_string(&IpcRequestPacket {
        cmd: &cmd,
        cwd,
        env: env_vars,
    })
    .expect("failed to serialize the command");
    cmd_string.push('\n');

    let mut server_started_by_us = false;
    let mut retry_count = 0;
    loop {
        match try_talking_to_server(&cmd_string, &cmd).await {
            Ok(_) => {
                return;
            }
            Err(ConnectError::OtherError(err)) => {
                println!("error occurred while connecting to server: {}", err);
            }
            Err(ConnectError::ServerNotStarted) => {
                // If the server is not started by us yet, let's try starting
                // it and wait for it to get ready. After that, we only retry
                // connecting to the server.
                if !server_started_by_us {
                    println!("starting the server as daemon...");
                    start_server_as_daemon();
                    server_started_by_us = true;
                }
            }
        }

        if retry_count <= 3 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            retry_count += 1;
        } else {
            println!("failed to talk to the server");
            return;
        }
    }
}

async fn try_talking_to_server(payload: &str, cmd: &dyn CommandClient) -> Result<(), ConnectError> {
    let mut stream = match UnixStream::connect(socket_path()?).await {
        Ok(stream) => stream,
        Err(err) => {
            if err.kind() == IoErrorKind::NotFound {
                return Err(ConnectError::ServerNotStarted);
            }
            return Err(ConnectError::OtherError(err.into()));
        }
    };

    // Send the command to server.
    stream.write_all(payload.as_bytes()).await?;

    // Create a buffer reader that can read the stream line by line,
    // since messages are delimited by newlines in our protocol.
    let stream_buf_read = BufReader::new(stream);
    let mut stream_lines = stream_buf_read.lines();

    // Receive all the contents from server until EOF.
    let mut stdout = io::stdout();
    while let Some(line) = stream_lines.next_line().await? {
        let pkt: OwnedIpcMessagePacket<serde_json::Value> = serde_json::from_str(&line)?;
        if let Some(output) = pkt.to_output() {
            stdout.write_all(output.as_bytes())?;
            stdout.flush()?;
        } else {
            if let Some(mut handler) = cmd.handler() {
                handler
                    .handle_response(pkt)
                    .await
                    .map_err(ConnectError::OtherError)?;
            }
            // End the program once we received the response packet.
            break;
        }
    }

    Ok(())
}

fn start_server_as_daemon() {
    let current_exe = env::current_exe().expect("failed to get current executable path");

    let pid = unsafe { libc::fork() };
    if pid != 0 {
        // The calling process can wait for the server to start and
        // retry its previous operation.
        return;
    }

    // Create a new session.
    let sid = unsafe { libc::setsid() };
    if sid == -1 {
        return;
    }

    unsafe {
        let root_dir_path = OsStr::new("/");
        libc::chdir(root_dir_path.as_bytes() as *const _ as *const i8);

        libc::umask(0);
    }

    // Daemonization is done. Now we can execute the program in server
    // mode, and exit the current process.
    process::Command::new(current_exe)
        .arg("--server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    process::exit(0);
}
