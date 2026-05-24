use crate::resp::{Resp, RespValue};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

pub(crate) struct StoreValue {
    value: RespValue,
    expiry: Option<DateTime<Utc>>,
}

pub(crate) struct Store {
    data: HashMap<String, StoreValue>,
}

impl Store {
    pub(crate) fn new() -> Self {
        Store {
            data: HashMap::new(),
        }
    }

    pub(crate) fn set(&mut self, key: String, value: RespValue, expiry: Option<DateTime<Utc>>) {
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
