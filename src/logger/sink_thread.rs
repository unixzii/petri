use std::io::Write;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

pub type BoxedWriter = Box<dyn Write + Send + 'static>;

pub struct SinkThread {
    writers: Vec<BoxedWriter>,
    rx: mpsc::Receiver<String>,
}

impl SinkThread {
    pub fn new(writers: Vec<BoxedWriter>, rx: mpsc::Receiver<String>) -> Self {
        Self { writers, rx }
    }

    pub fn start(self) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut writers = self.writers;
            let rx = self.rx;

            while let Ok(message) = rx.recv() {
                for writer in writers.iter_mut() {
                    // TODO: handle the write error. Maybe we should remove
                    // the bad writer if it failed too many times.
                    _ = writer.write_all(message.as_bytes());
                }
            }
        })
    }
}
