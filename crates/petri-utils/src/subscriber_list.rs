use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, RwLock, Weak};

pub struct SubscriberList<T: Send + Sync + 'static> {
    inner: Arc<Inner<T>>,
}

pub struct CancellationToken<T: Send + Sync + 'static> {
    inner: Weak<Inner<T>>,
    id: u64,
}

#[derive(Default)]
struct Inner<T> {
    id_seed: AtomicU64,
    map: RwLock<HashMap<u64, T>>,
}

impl<T: Send + Sync> SubscriberList<T> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                id_seed: Default::default(),
                map: Default::default(),
            }),
        }
    }

    pub fn subscribe(&self, subscriber: T) -> CancellationToken<T> {
        let id = self.inner.id_seed.fetch_add(1, AtomicOrdering::Relaxed);
        self.inner.map.write().unwrap().insert(id, subscriber);
        CancellationToken {
            inner: Arc::downgrade(&self.inner),
            id,
        }
    }

    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&T),
    {
        let map = self.inner.map.read().unwrap();
        for entry in map.values() {
            f(entry);
        }
    }

    #[allow(dead_code)]
    pub fn close(&self) {
        self.inner.map.write().unwrap().clear();
    }
}

impl<T: Send + Sync> Default for SubscriberList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + Sync> Drop for CancellationToken<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            _ = inner.map.write().unwrap().remove(&self.id)
        }
    }
}
