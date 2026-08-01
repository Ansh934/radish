use chrono::{Duration, Utc};
use rand::seq::IteratorRandom;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::rc::Rc;

use super::store_value::StoreValue;

/// A shared, single-threaded reference to the in-memory store.
///
/// Uses `Rc<RefCell<Store>>` because the server runs on a `tokio::task::LocalSet`
/// (single-threaded) and avoids the overhead of `Arc<Mutex<…>>`.
pub(crate) type SharedStore = Rc<RefCell<Store>>;

/// The in-memory key-value store.
pub(crate) struct Store {
    data: HashMap<Vec<u8>, StoreValue>,
    max_capacity: Option<usize>,
}

impl Store {
    /// Creates a new, empty `Store` wrapped in a `SharedStore`.
    #[allow(dead_code)]
    pub(crate) fn new() -> SharedStore {
        Rc::new(RefCell::new(Store {
            data: HashMap::new(),
            max_capacity: None,
        }))
    }

    pub(crate) fn with_capacity(max_capacity: usize) -> SharedStore {
        Rc::new(RefCell::new(Store {
            data: HashMap::new(),
            max_capacity: Some(max_capacity),
        }))
    }

    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    // ── String commands ──────────────────────────────────────────────────

    /// Inserts or updates `key` with a string `value`, optionally expiring
    /// after `expiry_ms` milliseconds from now.
    pub(crate) fn set(&mut self, key: &[u8], value: &[u8], expiry_ms: Option<i64>) {
        self.evict_if_needed();
        let expiry = expiry_ms.map(|ms| Utc::now() + Duration::milliseconds(ms));
        self.data.insert(
            key.to_vec(),
            StoreValue::new_string(value.to_vec(), expiry),
        );
    }

    /// Returns the string value for `key`, lazily removing expired keys.
    ///
    /// - `Ok(Some(bytes))` — live string value.
    /// - `Ok(None)` — key missing or expired.
    /// - `Err(WrongType)` — key exists but is not a string.
    pub(crate) fn get(&mut self, key: &[u8]) -> Result<Option<&[u8]>, crate::error::RadishError> {
        if self.remove_if_expired(key) {
            return Ok(None);
        }
        match self.data.get(key) {
            Some(sv) => Ok(Some(sv.as_string()?)),
            None => Ok(None),
        }
    }

    // ── Key-agnostic commands ────────────────────────────────────────────

    /// Returns the time-to-live of `key` in seconds:
    /// - `≥ 0` — remaining TTL
    /// - `-1` — key exists but has no expiry
    /// - `-2` — key does not exist (or has already expired)
    pub(crate) fn ttl(&mut self, key: &[u8]) -> i64 {
        if self.remove_if_expired(key) {
            return -2;
        }
        match self.data.get(key) {
            Some(sv) => match sv.expiry() {
                Some(expiry) => expiry.signed_duration_since(Utc::now()).num_seconds(),
                None => -1,
            },
            None => -2,
        }
    }

    pub(crate) fn del(&mut self, key: &[u8]) -> bool {
        self.data.remove(key).is_some()
    }

    pub(crate) fn expire(&mut self, key: &[u8], expiry_ms: i64) -> bool {
        if let Some(sv) = self.data.get_mut(key) {
            sv.set_expiry(Utc::now() + Duration::milliseconds(expiry_ms));
            true
        } else {
            false
        }
    }

    // ── Background maintenance ───────────────────────────────────────────

    /// Samples `n` random entries and removes any that have expired.
    /// Returns the fraction of sampled entries that were expired.
    pub(crate) fn cleanup_expired_entries(&mut self, n: usize) -> f64 {
        let mut rng = rand::rng();
        let now = Utc::now();

        let expired_keys: Vec<Vec<u8>> = self
            .data
            .iter()
            .sample(&mut rng, n)
            .into_iter()
            .filter_map(|(key, sv)| {
                if let Some(expiry) = sv.expiry() {
                    if expiry <= now {
                        return Some(key.clone());
                    }
                }
                None
            })
            .collect();
        let frac = expired_keys.len() as f64 / n as f64;

        for key in expired_keys {
            self.data.remove(&key);
        }
        frac
    }

    // ── AOF persistence ──────────────────────────────────────────────────

    pub(crate) fn dump_aof(&self) -> Result<(), std::io::Error> {
        use std::fs::OpenOptions;
        let aof_file_path = "appendonly.aof";
        let mut file: File = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(aof_file_path)?;
        let mut writer = std::io::BufWriter::new(&mut file);

        for (key, store_value) in &self.data {
            if store_value.is_expired() {
                continue;
            }
            store_value.dump_aof_command(key, &mut writer)?;
        }

        Ok(())
    }

    // ── Private helpers ──────────────────────────────────────────────────

    /// Removes `key` if it has expired. Returns `true` if a removal occurred.
    fn remove_if_expired(&mut self, key: &[u8]) -> bool {
        let expired = self
            .data
            .get(key)
            .map_or(false, |sv| sv.is_expired());
        if expired {
            self.data.remove(key);
        }
        expired
    }

    fn evict_if_needed(&mut self) {
        if let Some(max_capacity) = self.max_capacity {
            if self.data.len() >= max_capacity {
                self.data
                    .iter()
                    .choose(&mut rand::rng())
                    .map(|(key, _)| key.clone())
                    .map(|key_to_evict| {
                        self.data.remove(&key_to_evict);
                    });
            }
        }
    }
}
