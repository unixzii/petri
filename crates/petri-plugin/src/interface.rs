use crate::{Context, Result};

/// Information about an exited process.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProcessExitInfo {
    pid: u32,
    exit_code: i32,
}

/// A trait that plugins should implement to provide an entry point.
pub trait Plugin: Send {
    /// Called when the plugin is about to be loaded.
    ///
    /// Return `Ok(())` if the plugin was successfully loaded and can
    /// be used. No further plugin methods will be called after this
    /// method returns an error, and the object will be dropped.
    fn load(&mut self, cx: &Context) -> Result<()>;

    /// Called when a process has exited.
    fn handle_process_exit(&mut self, info: ProcessExitInfo, cx: &Context) -> Result<()> {
        _ = info;
        _ = cx;
        Ok(())
    }
}
