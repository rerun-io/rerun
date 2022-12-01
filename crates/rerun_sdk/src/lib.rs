//! The Rerun SDK
//!
//! Most operations go through the [`Sdk`] Singleton.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// Convert data into arrow types
pub mod arrow;

// Work with timestamps
pub mod time;
pub use time::log_time;

// Send data to a rerun session
mod session;
pub use self::session::Session;

mod global;
pub use self::global::global_session;

pub mod viewer;
