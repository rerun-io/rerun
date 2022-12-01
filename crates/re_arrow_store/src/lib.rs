//! This is how we store and index logging data.
//! TODO(john) better crate documentation.

mod util;

mod arrow_log_db;
pub use self::arrow_log_db::LogDb;

mod data_store;
pub use self::data_store::{ComponentName, ComponentNameRef, DataStore, TimeQuery};

pub use re_log_types::TimeInt; // to be polite
