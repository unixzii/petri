use std::io::{self, Result as IoResult, Write};

enum StdStream {
    Stdout(io::Stdout),
    Stderr(io::Stderr),
}

pub struct StdWriter(StdStream);

impl StdWriter {
    pub fn stdout() -> Self {
        Self(StdStream::Stdout(io::stdout()))
    }

    pub fn stderr() -> Self {
        Self(StdStream::Stderr(io::stderr()))
    }

    fn inner_mut(&mut self) -> &mut dyn Write {
        match &mut self.0 {
            StdStream::Stdout(stdout) => stdout,
            StdStream::Stderr(stderr) => stderr,
        }
    }
}

impl Default for StdWriter {
    fn default() -> Self {
        Self::stderr()
    }
}

impl Write for StdWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.inner_mut().write(buf)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.inner_mut().flush()
    }
}
