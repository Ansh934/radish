use chrono::{DateTime, Duration, Utc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A shared, single-threaded reference to the in-memory store.
///
/// Uses `Rc<RefCell<Store>>` because the server runs on a `tokio::task::LocalSet`
/// (single-threaded) and avoids the overhead of `Arc<Mutex<…>>`.
pub(crate) type SharedStore = Rc<RefCell<Store>>;

/// A single stored value with an optional expiry timestamp.
#[derive(Debug)]
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

    /// Inserts or updates `key` with `value`, optionally expiring after
    /// `expiry_ms` milliseconds from now.
    pub(crate) fn set(&mut self, key: &[u8], value: &[u8], expiry_ms: Option<i64>) {
        let expiry = expiry_ms.map(|ms| Utc::now() + Duration::milliseconds(ms));
        self.data
            .insert(key.to_vec(), StoreValue { value: value.to_vec(), expiry });
    }

    /// Returns `Some(&[u8])` for a live key, or `None` if the key is missing
    /// or has expired.
    pub(crate) fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.data.get(key).and_then(|sv| {
            if let Some(expiry) = sv.expiry
                && Utc::now() > expiry
            {
                return None; // expired
            }
            Some(sv.value.as_slice())
        })
    }

    /// Returns the time-to-live of `key` in seconds:
    /// - `≥ 0` — remaining TTL
    /// - `-1` — key exists but has no expiry
    /// - `-2` — key does not exist (or has already expired)
    pub(crate) fn ttl(&self, key: &[u8]) -> i64 {
        match self.data.get(key) {
            Some(sv) => match sv.expiry {
                Some(expiry) if expiry > Utc::now() => {
                    expiry.signed_duration_since(Utc::now()).num_seconds()
                }
                Some(_) => -2, // expired
                None => -1,    // no expiry set
            },
            None => -2, // key does not exist
        }
    }
}
