use std::io::Write;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use super::LoggerOp;

pub type BoxedWriter = Box<dyn Write + Send + 'static>;

pub struct SinkThread {
    writers: Vec<BoxedWriter>,
    rx: mpsc::Receiver<LoggerOp>,
}

impl SinkThread {
    pub fn new(writers: Vec<BoxedWriter>, rx: mpsc::Receiver<LoggerOp>) -> Self {
        Self { writers, rx }
    }

    pub fn start(self) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut this = self;
            while let Ok(op) = this.rx.recv() {
                this.execute_op(op);
            }
        })
    }

    fn execute_op(&mut self, op: LoggerOp) {
        match op {
            LoggerOp::Write(message) => {
                for writer in self.writers.iter_mut() {
                    // TODO: handle the write error. Maybe we should remove
                    // the bad writer if it failed too many times.
                    _ = writer.write_all(message.as_bytes());
                }
            }
            LoggerOp::SyncFlush(tx) => tx.send(()).expect("waiter released too early"),
        }
    }
}
