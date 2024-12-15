//! Specifies capability tokens, required by different parts of the code base.
//! These are tokens passed down the call tree, to explicitly allow different capabilities in different parts of the code base.
//!
//! For instance, the [`MainThreadToken`] is taken by argument in functions that needs to run on the main thread.
//! By requiring this token, you guarantee at compile-time that the function is only called on the main thread.
//!
//! All capability tokens should be created in the top-level of the call tree,
//! (i.e. in `fn main`) and passed down to all functions that require it.
//! That way you can be certain in what an area of code is allowed to do.
//!
//! See [`cap-std`](https://crates.io/crates/cap-std) for another capability-centric crate.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod main_thread_token;

pub use main_thread_token::MainThreadToken;
