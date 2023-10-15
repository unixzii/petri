use std::env;
use std::error::Error as StdError;
use std::ffi::OsStr;
use std::io::{self, ErrorKind as IoErrorKind, Write};
use std::os::unix::prelude::OsStrExt;
use std::process::{self, Command, Stdio};

use anyhow::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::control;

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
    let mut server_started_by_us = false;
    let mut retry_count = 0;
    loop {
        match try_talking_to_server(&args).await {
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

async fn try_talking_to_server(args: &Vec<String>) -> Result<(), ConnectError> {
    let mut stream = match UnixStream::connect(control::env::socket_path()?).await {
        Ok(stream) => stream,
        Err(err) => {
            if err.kind() == IoErrorKind::NotFound {
                return Err(ConnectError::ServerNotStarted);
            }
            return Err(ConnectError::OtherError(err.into()));
        }
    };

    // Serialize and send the args to server.
    let mut args_str = serde_json::to_string(args)?;
    args_str.push('\n');
    stream.write_all(args_str.as_bytes()).await?;

    // Receive all the contents from server until EOF.
    let mut buf = Vec::with_capacity(1024);
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
    Command::new(&current_exe)
        .arg("--server")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    process::exit(0);
}
