use std::env;
use std::error::Error as StdError;
use std::ffi::OsStr;
use std::io::{self, ErrorKind as IoErrorKind, Write};
use std::os::unix::prelude::OsStrExt;
use std::process::{self, Stdio};

use anyhow::Error;
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::control;
use crate::control::command::{Command, CommandClient};

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
    // Parse and serialize the command.
    let cmd = Command::parse_from(args);
    let mut cmd_string = serde_json::to_string(&control::cli::IpcRequestPacket { cmd: &cmd })
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
    let mut stream = match UnixStream::connect(control::env::socket_path()?).await {
        Ok(stream) => stream,
        Err(err) => {
            if err.kind() == IoErrorKind::NotFound {
                return Err(ConnectError::ServerNotStarted);
            }
            return Err(ConnectError::OtherError(err.into()));
        }
    };

    let handler = cmd.handler();
    let is_stream_mode = handler.is_none();

    // Send the command to server.
    if !is_stream_mode {
        // Request line for JSON-mode starts with a '>' character.
        stream.write_all(b">").await?;
    }
    stream.write_all(payload.as_bytes()).await?;

    // Prepare the buffer that can be used both in stream-mode and
    // JSON-mode.
    let mut buf = Vec::with_capacity(1024);

    if !is_stream_mode {
        tokio::io::copy(&mut stream, &mut buf).await?;
        let resp = String::from_utf8(buf)?;
        if let Err(err) = handler.unwrap().handle_response(&resp).await {
            return Err(ConnectError::OtherError(err));
        }
        return Ok(());
    }

    // Receive all the contents from server until EOF.
    let mut stdout = io::stdout();
    loop {
        let read_cnt = stream.read_buf(&mut buf).await?;
        if read_cnt == 0 {
            break;
        }
        stdout.write_all(&buf[0..read_cnt])?;
        buf.clear();
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
