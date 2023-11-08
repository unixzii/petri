use std::error::Error as StdError;
use std::fmt::Debug;
use std::result::Result as StdResult;

use anyhow::Error as AnyhowError;

pub struct Error {
    error: AnyhowError,
}

impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    fn from(value: E) -> Self {
        let error = AnyhowError::from(value);
        Self { error }
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

pub type Result<T> = StdResult<T, Error>;
