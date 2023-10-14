use std::io::ErrorKind as IoErrorKind;
use std::process::Stdio;
use std::sync::{Arc, Weak};

use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::runtime::Handle as TokioHandle;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{oneshot, watch};
use tokio::sync::{Mutex, RwLock};

use super::{Inner as ProcessManagerInner, ProcessManager};
use crate::util::subscriber_list::{self, SubscriberList};
use crate::util::{BufList, VecExt};

pub struct StartInfo {
    pub program: String,
    pub args: Option<Vec<String>>,
    pub cwd: String,
}

pub struct Process {
    inner: Arc<Inner>,
}

enum ExitCode {
    Pending(oneshot::Receiver<i32>),
    Completed(i32),
}

struct Inner {
    id: u32,
    exit_code: Mutex<ExitCode>,
    manager_inner: Weak<ProcessManagerInner>,

    kill_signal: watch::Sender<bool>,

    output_buf_list: RwLock<BufList>,
    output_subscribers: SubscriberList<UnboundedSender<Vec<u8>>>,
}

impl Process {
    pub(super) fn spawn(start_info: &StartInfo, manager: &ProcessManager) -> Result<Self> {
        let mut command = Command::new(&start_info.program);

        if let Some(args) = &start_info.args {
            command.args(args);
        }

        let mut child = command
            .current_dir(&start_info.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let Some(id) = child.id() else {
            return Err(anyhow!("process exited before being tracked"));
        };

        let Some(stdout) = child.stdout.take() else {
            return Err(anyhow!("cannot get stdout pipe"));
        };
        let Some(stderr) = child.stderr.take() else {
            return Err(anyhow!("cannot get stderr pipe"));
        };

        let (kill_signal_tx, kill_signal_rx) = watch::channel(false);
        let (exit_code_tx, exit_code_rx) = oneshot::channel();
        let inner = Arc::new(Inner {
            id,
            exit_code: Mutex::new(ExitCode::Pending(exit_code_rx)),
            manager_inner: Arc::downgrade(&manager.inner),
            kill_signal: kill_signal_tx,
            output_buf_list: Default::default(),
            output_subscribers: Default::default(),
        });
        inner.monit_process(stdout, stderr, child, kill_signal_rx, exit_code_tx);

        Ok(Self { inner })
    }

    pub fn id(&self) -> u32 {
        self.inner.id
    }

    pub async fn kill(&self) -> i32 {
        _ = self.inner.kill_signal.send(true);

        let mut exit_code_lock = self.inner.exit_code.lock().await;
        // Take the current value of `exit_code` and put a placeholder.
        let mut exit_code = ExitCode::Completed(0);
        std::mem::swap(&mut *exit_code_lock, &mut exit_code);

        let exit_code = match exit_code {
            ExitCode::Pending(rx) => rx
                .await
                .expect("`exit_code` sender should not drop without sending values"),
            ExitCode::Completed(code) => code,
        };

        *exit_code_lock = ExitCode::Completed(exit_code);
        exit_code
    }

    pub async fn attach_output_channel(
        &self,
        sender: UnboundedSender<Vec<u8>>,
    ) -> subscriber_list::CancellationToken<UnboundedSender<Vec<u8>>> {
        let output_buf_list = self.inner.output_buf_list.read().await;

        let cached_contents = output_buf_list.peek();
        if !cached_contents.is_empty() {
            _ = sender.send(cached_contents);
        }

        let token = self.inner.output_subscribers.subscribe(sender);
        drop(output_buf_list);

        token
    }
}

const STDIO_BUF_SIZE: usize = 1024;

impl Inner {
    fn monit_process(
        self: &Arc<Self>,
        stdout: ChildStdout,
        stderr: ChildStderr,
        mut child: Child,
        mut kill_signal: watch::Receiver<bool>,
        exit_code: oneshot::Sender<i32>,
    ) {
        let rt_handle = TokioHandle::current();

        self.read_stdio(&rt_handle, stdout);
        self.read_stdio(&rt_handle, stderr);

        let process_inner = Arc::clone(self);
        rt_handle.spawn(async move {
            loop {
                let exit_status = tokio::select! {
                    exit_status = child.wait() => {
                        exit_status.expect("failed to wait child")
                    },
                    _ = kill_signal.changed() => {
                        if *kill_signal.borrow_and_update() {
                            _ = child.start_kill();
                        }
                        continue;
                    }
                };
                // TODO: the exit code is simulated for processes that were killed by signals.
                let real_exit_code = exit_status.code().unwrap_or(1);
                _ = exit_code.send(real_exit_code);

                // Also close currently active output subscribers.
                process_inner.output_subscribers.close();

                if let Some(manager_inner) = process_inner.manager_inner.upgrade() {
                    manager_inner
                        .handle_process_exit(process_inner.id, real_exit_code)
                        .await;
                }
                return;
            }
        });
    }

    fn read_stdio<R: AsyncRead + Send + Unpin + 'static>(
        self: &Arc<Self>,
        rt_handle: &TokioHandle,
        mut pipe: R,
    ) {
        let self_clone = Arc::clone(self);
        rt_handle.spawn(async move {
            let mut buf = Vec::with_capacity(STDIO_BUF_SIZE);
            loop {
                match pipe.read_buf(&mut buf).await {
                    Ok(cnt) => {
                        if cnt == 0 {
                            // No more data to read.
                            break;
                        }

                        if buf.len() >= STDIO_BUF_SIZE {
                            self_clone.write_output(buf.clone()).await;
                            buf.clear();
                        } else if let Some(buf) = buf.split_off_with(|b| *b == b'\n') {
                            self_clone.write_output(buf).await;
                        }
                    }
                    Err(err) => {
                        if err.kind() != IoErrorKind::Interrupted {
                            todo!();
                        }
                    }
                }
            }

            if !buf.is_empty() {
                self_clone.write_output(buf).await;
            }
        });
    }

    async fn write_output(self: &Arc<Self>, buf: Vec<u8>) {
        let mut output_buf_list = self.output_buf_list.write().await;
        output_buf_list.push(buf.clone());

        self.output_subscribers.for_each(|sender| {
            _ = sender.send(buf.clone());
        });

        // Use `output_buf_list` lock as a barrier to make sure that we
        // will not send data in the middle of a subscribing procedure.
        drop(output_buf_list);
    }
}
