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

/// Rerun Cloud permissions
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Permission {
    /// User can read data.
    #[serde(rename = "read")]
    Read,

    /// User can both read and write data.
    #[serde(rename = "read-write")]
    ReadWrite,

    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, thiserror::Error)]
#[error("invalid permission")]
pub struct InvalidPermission;

impl std::str::FromStr for Permission {
    type Err = InvalidPermission;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "read" => Ok(Self::Read),
            "read-write" => Ok(Self::ReadWrite),
            _ => Err(InvalidPermission),
        }
    }
}

pub mod credentials;

#[cfg(all(feature = "cli", feature = "oauth", not(target_arch = "wasm32")))]
pub mod cli;

#[cfg(feature = "oauth")]
pub mod oauth;

#[cfg(all(feature = "oauth", not(target_arch = "wasm32")))]
pub mod callback_server;

#[cfg(not(target_arch = "wasm32"))]
pub use error::Error;
#[cfg(all(feature = "oauth", not(target_arch = "wasm32")))]
pub use oauth::login_flow::{DeviceCodeFlow, OauthLoginFlow};
#[cfg(not(target_arch = "wasm32"))]
pub use provider::{Claims, RedapProvider, SecretKey, VerificationOptions};
pub use service::client;
#[cfg(not(target_arch = "wasm32"))]
pub use service::server;
pub use token::{
    DEFAULT_ALLOWED_HOSTS, HostMismatchError, INSECURE_SKIP_HOST_CHECK_ENV, Jwt, JwtDecodeError,
    TokenError, host_matches_pattern, token_allowed_for_host,
};

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

mod wasm_compat;
