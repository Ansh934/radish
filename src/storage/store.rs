use chrono::{DateTime, Duration, Utc};
use rand::seq::IteratorRandom;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A shared, single-threaded reference to the in-memory store.
///
/// Uses `Rc<RefCell<Store>>` because the server runs on a `tokio::task::LocalSet`
/// (single-threaded) and avoids the overhead of `Arc<Mutex<…>>`.
pub(crate) type SharedStore = Rc<RefCell<Store>>;

/// A single stored value with an optional expiry timestamp.

pub(crate) struct StoreValue {
    value: Vec<u8>,
    expiry: Option<DateTime<Utc>>,
}

/// The in-memory key-value store.
pub(crate) struct Store {
    data: HashMap<Vec<u8>, StoreValue>,
}

impl Store {
    /// Creates a new, empty `Store` wrapped in a `SharedStore`.
    pub(crate) fn new() -> SharedStore {
        Rc::new(RefCell::new(Store {
            data: HashMap::new(),
        }))
    }
    
    pub(crate) fn len(&self) -> usize {
        self.data.len()
    }

    /// Inserts or updates `key` with `value`, optionally expiring after
    /// `expiry_ms` milliseconds from now.
    pub(crate) fn set(&mut self, key: &[u8], value: &[u8], expiry_ms: Option<i64>) {
        let expiry = expiry_ms.map(|ms| Utc::now() + Duration::milliseconds(ms));
        self.data.insert(
            key.to_vec(),
            StoreValue {
                value: value.to_vec(),
                expiry,
            },
        );
    }

    /// Returns `Some(&[u8])` for a live key, or `None` if the key is missing
    /// or has expired.
    pub(crate) fn get(&mut self, key: &[u8]) -> Option<&[u8]> {
        let is_expired = self.data.get(key).map_or(false, |sv| {
            sv.expiry.map_or(false, |expiry| expiry <= Utc::now())
        });
        if is_expired {
            self.data.remove(key);
            None
        } else {
            self.data.get(key).map(|sv| sv.value.as_slice())
        }
    }

    /// Returns the time-to-live of `key` in seconds:
    /// - `≥ 0` — remaining TTL
    /// - `-1` — key exists but has no expiry
    /// - `-2` — key does not exist (or has already expired)
    pub(crate) fn ttl(&mut self, key: &[u8]) -> i64 {
        let mut is_expired = false;
        let response = match self.data.get(key) {
            Some(sv) => match sv.expiry {
                Some(expiry) if expiry > Utc::now() => {
                    expiry.signed_duration_since(Utc::now()).num_seconds()
                }
                Some(_) => {
                    is_expired = true;
                    -2
                } // expired
                None => -1, // no expiry set
            },
            None => -2, // key does not exist
        };

        if is_expired {
            self.data.remove(key);
        }
        response
    }

    pub(crate) fn del(&mut self, key: &[u8]) -> bool {
        self.data.remove(key).is_some()
    }

    pub(crate) fn expire(&mut self, key: &[u8], expiry_ms: i64) -> bool {
        if let Some(sv) = self.data.get_mut(key) {
            sv.expiry = Some(Utc::now() + Duration::milliseconds(expiry_ms));
            true
        } else {
            false
        }
    }

    // check n random entries for expiry and remove them if expired
    pub(crate) fn cleanup_expired_entries(&mut self, n: usize) -> f64 {
        let mut rng = rand::rng();
        let now = Utc::now();

        let expired_keys: Vec<Vec<u8>> = self
            .data
            .iter()
            .sample(&mut rng, n)
            .into_iter()
            .filter_map(|(key, sv)| {
                if let Some(expiry) = sv.expiry {
                    if expiry <= now {
                        return Some(key.clone()); // Only clone if we are deleting it!
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
}
