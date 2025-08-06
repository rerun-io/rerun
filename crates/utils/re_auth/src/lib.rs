//! Basic authentication helpers for Rerun.
//!
//! Currently, this crate provides a simple [`Jwt`]-based authentication scheme on
//! top of a rudimentary [`RedapProvider`] that uses a symmetric key to _both_
//! generate and sign tokens.
//!
//! **Warning!** This approach should only be seen as a stop-gap until we have
//! integration of _real_ identity-providers, most likely based on `OpenID` Connect.

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

/// The error message in Tonic's gRPC status when the token is malformed or invalid in some way.
///
/// The associated status code will always be `Unauthenticated`.
pub const ERROR_MESSAGE_MALFORMED_CREDENTIALS: &str = "malformed auth token";

/// The error message in Tonic's gRPC status when no token was found.
///
/// The associated status code will always be `Unauthenticated`.
pub const ERROR_MESSAGE_MISSING_CREDENTIALS: &str = "missing credentials";

/// The error message in Tonic's gRPC status when a _valid token_ did not have the required permissions.
///
/// The associated status code will always be `Unauthenticated`.
pub const ERROR_MESSAGE_INVALID_CREDENTIALS: &str = "invalid credentials";
