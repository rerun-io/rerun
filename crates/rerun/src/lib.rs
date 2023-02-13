//! The main Rerun library and binary.
//!
//! ## Examples
//! See <https://github.com/rerun-io/rerun/tree/main/examples/rust>.
//!
//! ## Library
//! See [`Session`] and [`MsgSender`].
//!
//! ## Binary
//! This can act either as a server, a viewer, or both, depending on which options you use when you start it.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#![warn(missing_docs)] // Let's keep the this crate well-documented!

mod run;

pub use run::{run, CallSource};

// NOTE: Have a look at `re_sdk/src/lib.rs` for an accurate listing of all these symbols.
#[cfg(feature = "sdk")]
pub use re_sdk::*;
