use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::PathBuf;
use std::str::FromStr;

#[cfg(target_os = "macos")]
pub fn socket_path() -> Result<PathBuf> {
    let base_metadata = fs::metadata("/tmp")?;
    if !base_metadata.is_dir() {
        return Err(Error::new(
            ErrorKind::Other,
            "the location for socket is not available",
        ));
    }
    Ok(PathBuf::from_str("/tmp/petri.sock")
        .expect("creating path from literal string should success"))
}

#[cfg(not(target_os = "macos"))]
pub fn socket_path() -> Result<PathBuf> {
    compile_error!("target platform not supported")
}
