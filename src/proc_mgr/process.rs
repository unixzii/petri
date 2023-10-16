use std::io::ErrorKind as IoErrorKind;
use std::process::Stdio;
use std::sync::{Arc, Weak};

use anyhow::Result;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{oneshot, watch, Mutex, RwLock};
use tokio::task;

use super::Inner as ProcessManagerInner;
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

enum State {
    Running(oneshot::Sender<()>, watch::Receiver<Option<i32>>),
    Terminating(watch::Receiver<Option<i32>>),
    Terminated(i32),
    Placeholder,
}

struct Inner {
    id: u32,
    state: Mutex<State>,
    manager_inner: Weak<ProcessManagerInner>,

    output_buf_list: RwLock<BufList>,
    output_subscribers: SubscriberList<UnboundedSender<Vec<u8>>>,
}

impl Process {
    pub(super) fn spawn(
        start_info: &StartInfo,
        manager_inner: &Arc<ProcessManagerInner>,
    ) -> Result<Self> {
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

        let (kill_signal_tx, kill_signal_rx) = oneshot::channel();
        let (exit_code_tx, exit_code_rx) = watch::channel(None);
        let inner = Arc::new(Inner {
            id,
            state: Mutex::new(State::Running(kill_signal_tx, exit_code_rx)),
            manager_inner: Arc::downgrade(manager_inner),
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
        let mut state = self.inner.state.lock().await;

        // Take the current state and put a placeholder.
        let mut current_state = State::Placeholder;
        std::mem::swap(&mut *state, &mut current_state);

        let mut exit_code_rx = match current_state {
            State::Running(kill_signal_tx, exit_code_rx) => {
                _ = kill_signal_tx.send(());
                *state = State::Terminating(exit_code_rx.clone());
                exit_code_rx
            }
            State::Terminating(exit_code_rx) => {
                *state = State::Terminating(exit_code_rx.clone());
                exit_code_rx
            }
            State::Terminated(exit_code) => {
                *state = State::Terminated(exit_code);
                return exit_code;
            }
            State::Placeholder => {
                unreachable!()
            }
        };

        drop(state);

        // Wait for the exit code and update the state.
        exit_code_rx
            .changed()
            .await
            .expect("`exit_code` sender should not drop without sending values");
        let exit_code = exit_code_rx
            .borrow_and_update()
            .expect("the sent value should not be empty");
        *self.inner.state.lock().await = State::Terminated(exit_code);

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
        kill_signal: oneshot::Receiver<()>,
        exit_code_tx: watch::Sender<Option<i32>>,
    ) {
        self.read_stdio(stdout);
        self.read_stdio(stderr);

        let process_inner = Arc::clone(self);
        task::spawn(async move {
            let exit_status = tokio::select! {
                exit_status = child.wait() => {
                    Some(exit_status.expect("failed to wait child"))
                },
                kill_signal = kill_signal => {
                    if kill_signal.is_ok() {
                        _ = child.start_kill();
                    }
                    None
                }
            };

            let exit_status = if let Some(exit_status) = exit_status {
                exit_status
            } else {
                // The process is killed but not terminated yet, we need
                // to wait it again.
                child.wait().await.expect("failed to wait child")
            };

            // TODO: the exit code is simulated for processes that were killed by signals.
            let exit_code = exit_status.code().unwrap_or(1);
            _ = exit_code_tx.send(Some(exit_code));

            if let Some(manager_inner) = process_inner.manager_inner.upgrade() {
                manager_inner
                    .handle_process_exit(process_inner.id, exit_code)
                    .await;
            }
        });
    }

    fn read_stdio<R: AsyncRead + Send + Unpin + 'static>(self: &Arc<Self>, mut pipe: R) {
        let self_clone = Arc::clone(self);
        task::spawn(async move {
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
