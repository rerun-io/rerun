//! Basic authentication helpers for Rerun.
//!
//! Currently, this crate provides a simple [`Jwt`]-based authentication scheme on
//! top of a rudimentary [`RedapProvider`] that uses a symmetric key to _both_
//! generate and sign tokens.
//!
//! **Warning!** This approach should only be seen as a stop-gap until we have
//! integration of _real_ identity-providers, most likely based on `OpenID` Connect.

pub use error::Error;
pub use provider::{Claims, RedapProvider, VerificationOptions};
pub use service::*;
pub use token::{Jwt, TokenError};

mod error;
mod provider;
mod service;
mod token;
