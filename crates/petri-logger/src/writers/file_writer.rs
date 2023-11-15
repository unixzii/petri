use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;

use chrono::{DateTime, Datelike, Local};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("the file is not rotated")]
    NotRotated,
    #[error("failed to create file")]
    FailedToCreateFile(io::Error),
}

pub struct FileWriter {
    file_path_builder: FilePathBuilder,
    active_file: Option<File>,
    needs_rotation: Arc<AtomicBool>,
    rotation_driver: Option<Box<dyn AnyRotationDriver>>,
}

impl FileWriter {
    pub fn new(file_path_builder: FilePathBuilder) -> Result<Self, Error> {
        let mut this = Self {
            file_path_builder,
            active_file: None,
            needs_rotation: Arc::new(AtomicBool::new(false)),
            rotation_driver: None,
        };
        this.try_rotate()?;
        Ok(this)
    }

    pub fn set_rotation_driver<D>(&mut self, mut driver: D)
    where
        D: RotationDriver + 'static,
    {
        let needs_rotation = Arc::clone(&self.needs_rotation);
        driver.register(move || {
            needs_rotation.store(true, AtomicOrdering::Relaxed);
        });
        self.rotation_driver = Some(Box::new(driver));
    }

    #[cold]
    pub fn try_rotate(&mut self) -> Result<(), Error> {
        // If there is already an active file, we need to rotate the file
        // path first. Otherwise we can create the file directly.
        if self.active_file.is_some() && !self.file_path_builder.rotate_if_needed() {
            return Err(Error::NotRotated);
        }

        let mut last_io_error = None;
        // A heuristic approach to avoid infinite failure loop.
        for _ in 0..100 {
            let path = self.file_path_builder.make_path();
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(file) => {
                    if let Some(mut old_file) = self.active_file.replace(file) {
                        _ = old_file.flush();
                    }
                    return Ok(());
                }
                Err(err) => {
                    last_io_error = Some(err);
                }
            };
        }

        Err(Error::FailedToCreateFile(last_io_error.unwrap()))
    }
}

impl Drop for FileWriter {
    fn drop(&mut self) {
        if let Some(driver) = self.rotation_driver.as_mut() {
            driver.cancel();
        }
    }
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self
            .needs_rotation
            .fetch_and(false, AtomicOrdering::Relaxed)
        {
            // We can just ignore the error since it keeps the
            // active file unchanged if an error occurred.
            _ = self.try_rotate();
        }

        self.active_file
            .as_mut()
            .expect("expected an active file")
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.active_file
            .as_mut()
            .expect("expected an active file")
            .flush()
    }
}

pub struct FilePathBuilder {
    base_path: PathBuf,
    prefix: String,
    ext: String,
    last_date: DateTime<Local>,
    conflict_counter: u64,
}

impl FilePathBuilder {
    pub fn new<P>(base_path: P, prefix: &str, ext: &str) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            base_path: base_path.as_ref().to_owned(),
            prefix: prefix.to_owned(),
            ext: ext.to_owned(),
            last_date: Local::now(),
            conflict_counter: 0,
        }
    }

    fn make_path(&mut self) -> PathBuf {
        let date_string = self.last_date.format("%Y%m%d");
        let suffix = if self.conflict_counter == 0 {
            format!("-{date_string}")
        } else {
            let discriminator = self.conflict_counter + 1;
            format!("-{date_string}-{discriminator}")
        };
        self.conflict_counter += 1;

        let file_name = format!("{}{}.{}", self.prefix, suffix, self.ext);

        let mut path = self.base_path.to_owned();
        path.push(file_name);

        path
    }

    fn rotate_if_needed(&mut self) -> bool {
        let now = Local::now();
        if self.last_date.day() == now.day()
            && self.last_date.month() == now.month()
            && self.last_date.year() == now.year()
        {
            return false;
        }

        self.last_date = now;
        self.conflict_counter = 0;

        true
    }
}

/// A trait for providing capabilities to drive the log rotation.
pub trait RotationDriver: Send {
    /// Registers a callback to be invoked when the writer should
    /// check whether it needs to rotate the log.
    ///
    /// The driver will invoke the specified callback periodically
    /// in an arbitrary thread. The interval between invocations
    /// depends on the driver implementation.
    ///
    /// When the driver is dropped or [`RotateDriver::cancel`] method
    /// is called, the registered callback will be cancelled.
    fn register<C>(&mut self, callback: C)
    where
        C: Fn() + Send + 'static;

    /// Cancels all the registered callbacks, and none of them will
    /// be invoked after this method returns.
    fn cancel(&mut self);
}

trait AnyRotationDriver: Send {
    fn cancel(&mut self);
}

impl<T: RotationDriver> AnyRotationDriver for T {
    fn cancel(&mut self) {
        RotationDriver::cancel(self)
    }
}

#[cfg(test)]
mod tests {
    use super::FilePathBuilder;

    #[test]
    fn test_file_path_builder() {
        let mut builder = FilePathBuilder::new("/tmp", "hello", "log");
        let path1 = builder.make_path();
        assert_eq!(path1.extension().unwrap(), "log");
        assert!(path1
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("hello-"));

        let path2 = builder.make_path();
        assert_ne!(path1, path2);
    }
}
