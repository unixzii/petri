use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;

use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use petri_logger::writers::file_writer;

type BoxedCallback = Box<dyn Fn() + Send>;
type SharedCallbackList = Arc<Mutex<Vec<BoxedCallback>>>;

#[derive(Default)]
pub struct CallbackRegistry {
    id_seed: AtomicU64,
    lists: Mutex<HashMap<u64, SharedCallbackList>>,
}

impl CallbackRegistry {
    pub fn make_driver(&self) -> RotationDriver {
        let id = self.id_seed.fetch_add(1, AtomicOrdering::Relaxed);
        let callback_list = SharedCallbackList::default();
        self.lists.lock().insert(id, Arc::clone(&callback_list));
        RotationDriver { id, callback_list }
    }

    fn release_driver(&self, driver: &RotationDriver) {
        let id = driver.id;
        self.lists.lock().remove(&id);
    }

    pub fn notify_all(&self) {
        for list in self.lists.lock().values() {
            list.lock().iter().for_each(|cb| {
                cb();
            });
        }
    }
}

static ROTATION_CALLBACK_REGISTRY: OnceCell<CallbackRegistry> = OnceCell::new();

pub fn rotation_callback_registry() -> &'static CallbackRegistry {
    ROTATION_CALLBACK_REGISTRY.get_or_init(CallbackRegistry::default)
}

pub struct RotationDriver {
    id: u64,
    callback_list: SharedCallbackList,
}

impl file_writer::RotationDriver for RotationDriver {
    fn register(&self, callback: Box<dyn Fn() + Send>) {
        self.callback_list.lock().push(Box::new(callback));
    }

    fn cancel(&self) {
        self.callback_list.lock().clear();
    }
}

impl Drop for RotationDriver {
    fn drop(&mut self) {
        rotation_callback_registry().release_driver(self);
    }
}
