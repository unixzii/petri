use crate::{Context, Plugin, ProcessExitInfo};

#[repr(C)]
pub struct RawContext {}

impl RawContext {
    pub fn to_context(&self) -> Context {
        Context {}
    }
}

#[repr(C)]
pub struct PluginVTable {
    load: extern "C" fn(user_data: *mut (), cx: *const RawContext) -> bool,
    handle_process_exit: extern "C" fn(
        user_data: *mut (),
        info: *const ProcessExitInfo,
        cx: *const RawContext,
    ) -> bool,
    drop: extern "C" fn(user_data: *mut ()),
}

#[repr(C)]
pub struct PluginInstance {
    user_data: *mut (),
    vtable: PluginVTable,
}

impl Drop for PluginInstance {
    fn drop(&mut self) {
        if self.user_data.is_null() {
            return;
        }
        (self.vtable.drop)(self.user_data);
        self.user_data = std::ptr::null_mut();
    }
}

macro_rules! convert_err {
    ($v:expr) => {
        $v.is_ok()
    };
}

pub fn new_plugin_instance<T>(plugin: T) -> PluginInstance
where
    T: Plugin + 'static,
{
    extern "C" fn load_impl<T: Plugin>(user_data: *mut (), cx: *const RawContext) -> bool {
        let plugin_ref = unsafe { &mut *(user_data as *mut T) };
        let cx = unsafe { (*cx).to_context() };
        convert_err!(plugin_ref.load(&cx))
    }

    extern "C" fn handle_process_exit<T: Plugin>(
        user_data: *mut (),
        info: *const ProcessExitInfo,
        cx: *const RawContext,
    ) -> bool {
        let plugin_ref = unsafe { &mut *(user_data as *mut T) };
        let info = unsafe { *info };
        let cx = unsafe { (*cx).to_context() };
        convert_err!(plugin_ref.handle_process_exit(info, &cx))
    }

    extern "C" fn drop_impl<T>(user_data: *mut ()) {
        let boxed = unsafe { Box::<T>::from_raw(user_data as *mut _) };
        drop(boxed);
    }

    let boxed = Box::new(plugin);
    PluginInstance {
        user_data: Box::into_raw(boxed) as *mut _,
        vtable: PluginVTable {
            load: load_impl::<T>,
            handle_process_exit: handle_process_exit::<T>,
            drop: drop_impl::<T>,
        },
    }
}
