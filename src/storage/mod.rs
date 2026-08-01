//! In-memory key-value storage layer.
//!
//! Intentionally simple: a `HashMap` behind `Rc<RefCell<…>>`, sized for
//! single-threaded async use.  Swap this module out to add persistence,
//! eviction, or multi-threaded access without touching any other layer.

mod store;
mod store_value;

pub(crate) use store::{SharedStore, Store};