use crate::resp::RespValue;
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) type SharedStore = Rc<RefCell<Store>>;

#[derive(Debug)]
pub(crate) struct StoreValue {
    value: RespValue,
    expiry: Option<DateTime<Utc>>,
}

pub(crate) struct Store {
    data: HashMap<Bytes, StoreValue>,
}

impl Store {
    pub(crate) fn new() -> SharedStore {
        Rc::new(RefCell::new(Store {
            data: HashMap::new(),
        }))
    }

    pub(crate) fn set(&mut self, key: Bytes, value: RespValue, expiry_ms: Option<i64>) {
        let expiry = match expiry_ms {
            Some(ms) => Some(Utc::now() + Duration::milliseconds(ms)),
            None => None,
        };
        self.data.insert(key, StoreValue { value, expiry });
    }

    pub(crate) fn get(&self, key: &Bytes) -> Option<&RespValue> {
        self.data.get(key).and_then(|store_value| {
            println!(
                "Found value: {:?} with expiry: {:?}",
                store_value.value, store_value.expiry
            );
            if let Some(expiry) = store_value.expiry {
                if Utc::now() > expiry {
                    return None; // expired
                }
            }
            Some(&store_value.value)
        })
    }
    
    pub(crate) fn ttl(&self, key: &Bytes) -> i64 {
        // Returns:
        // >= 0 the remaining time to live in seconds
        // -1 if the key exists but has no associated expiry time
        // -2 if the key does not exist
        match self.data.get(key) {
            Some(store_value) => match store_value.expiry {
                Some(expiry) if expiry > Utc::now() => expiry.signed_duration_since(Utc::now()).num_seconds(),
                Some(_) => -2, // expired
                None => -1, // no expiry
            },
            None => -2, // key does not exist
        }
    }
}
