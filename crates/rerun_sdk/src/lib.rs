//! The Rerun SDK
//!
//! Most operations go through the [`Sdk`] Singleton.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod sdk;
pub use self::sdk::Sdk;
