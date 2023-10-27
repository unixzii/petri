mod sink_thread;
pub mod writers;

use std::env;
use std::path::Path;
use std::sync::mpsc;

use sink_thread::{BoxedWriter, SinkThread};

#[derive(Default)]
pub struct LoggerBuilder {
    std_writer: Option<writers::StdWriter>,
}

impl LoggerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn enable_stdout(mut self) -> Self {
        self.std_writer = Some(writers::StdWriter::stdout());
        self
    }

    pub fn enable_stderr(mut self) -> Self {
        self.std_writer = Some(writers::StdWriter::stderr());
        self
    }

    pub fn build(self) -> Logger {
        let mut writers: Vec<BoxedWriter> = vec![];

        if let Some(std_writer) = self.std_writer {
            let std_writer = Box::new(std_writer);
            writers.push(std_writer);
        }

        let (tx, rx) = mpsc::channel();

        SinkThread::new(writers, rx).start();

        let exec_name = env::args()
            .next()
            .and_then(|arg| {
                Path::new(&arg)
                    .file_name()
                    .and_then(|path| path.to_str())
                    .map(|s| s.to_owned())
            })
            .unwrap_or_default();

        Logger {
            tx,
            exec_name,
            pid: std::process::id(),
        }
    }
}

pub struct Logger {
    tx: mpsc::Sender<String>,
    exec_name: String,
    pid: u32,
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        // TODO: implement level filter.
        true
    }

    fn log(&self, record: &log::Record) {
        let now = chrono::Local::now();
        let formatted_now = now.format("%Y-%m-%d %T%:::z");
        let exec_name = &self.exec_name;
        let pid = self.pid;

        // Format and indent the args string.
        let mut args = format!("{}", record.args());
        args = args.replace('\n', "\n\t");

        let message = format!(
            "{formatted_now} {exec_name}[{pid}] {}: {}:{}: {args}\n",
            &record.level().as_str()[0..1],
            record.file().unwrap_or("<unknown>"),
            record.line().unwrap_or_default(),
        );

        _ = self.tx.send(message);
    }

    fn flush(&self) {
        // This is no-op intentionally.
    }
}
