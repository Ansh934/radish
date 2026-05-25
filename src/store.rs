use crate::resp::RespValue;
use chrono::{DateTime, Duration, Utc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) type SharedStore = Rc<RefCell<Store>>;

pub(crate) struct StoreValue {
    value: RespValue,
    expiry: Option<DateTime<Utc>>,
}

pub(crate) struct Store {
    data: HashMap<String, StoreValue>,
}

impl Store {
    pub(crate) fn new() -> SharedStore {
        Rc::new(RefCell::new(Store {
            data: HashMap::new(),
        }))
    }

    pub(crate) fn clone_shared(store: &SharedStore) -> SharedStore {
        Rc::clone(store)
    }

    pub(crate) fn set(&mut self, key: String, value: RespValue, expiry: Option<i64>) {
        let expiry = match expiry {
            Some(seconds) => Some(Utc::now() + Duration::seconds(seconds)),
            None => None,
        };
        self.data.insert(key, StoreValue { value, expiry });
    }

    pub(crate) fn get(&self, key: &str) -> Option<&RespValue> {
        self.data.get(key).and_then(|store_value| {
            if let Some(expiry) = store_value.expiry {
                if Utc::now() > expiry {
                    return None; // expired
                }
            }
            Some(&store_value.value)
        })
    }
}
