use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Utc, Duration};

#[derive(Clone)]
pub struct CacheEntry<T> {
    pub data: T,
    pub expires_at: DateTime<Utc>,
}

pub struct Cache<T: Clone> {
    store: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
    ttl: Duration,
}

impl<T: Clone> Cache<T> {
    pub fn new(ttl_days: i64) -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::days(ttl_days),
        }
    }

    pub fn get(&self, key: &str) -> Option<T> {
        let store = self.store.read().unwrap();
        if let Some(entry) = store.get(key) {
            if entry.expires_at > Utc::now() {
                return Some(entry.data.clone());
            }
        }
        None
    }

    pub fn set(&self, key: String, data: T) {
        let mut store = self.store.write().unwrap();
        store.insert(
            key,
            CacheEntry {
                data,
                expires_at: Utc::now() + self.ttl,
            },
        );
    }

    pub fn clear_expired(&self) {
        let mut store = self.store.write().unwrap();
        let now = Utc::now();
        store.retain(|_, entry| entry.expires_at > now);
    }
}
