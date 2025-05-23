//! Basic authentication helpers for Rerun.
//!
//! Currently, this crate provides a simple [`Jwt`]-based authentication scheme on
//! top of a rudimentary [`RedapProvider`] that uses a symmetric key to _both_
//! generate and sign tokens.
//!
//! **Warning!** This approach should only be seen as a stop-gap until we have
//! integration of _real_ identity-providers, most likely based on OpenID Connect.

#[cfg(not(target_arch = "wasm32"))]
mod error;
#[cfg(not(target_arch = "wasm32"))]
mod provider;

mod service;
mod token;

pub use service::client;
pub use token::{Jwt, TokenError};

#[cfg(not(target_arch = "wasm32"))]
pub use error::Error;
#[cfg(not(target_arch = "wasm32"))]
pub use provider::{Claims, RedapProvider, VerificationOptions};
#[cfg(not(target_arch = "wasm32"))]
pub use service::server;
