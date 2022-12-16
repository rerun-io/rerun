//! The Rerun SDK
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// Work with timestamps
pub mod time;
pub use time::log_time;

// Send data to a rerun session
mod session;
pub use self::session::Session;

mod global;
pub use self::global::global_session;

pub mod viewer;
