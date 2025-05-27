//! This module contains auth middleware for [`tonic`] services.

pub mod client;

#[cfg(not(target_arch = "wasm32"))]
pub mod server;

/// The metadata key used in the metadata of the gRPC request to store the token.
const AUTHORIZATION_KEY: &str = "authorization";

/// The prefix for the token in the metadata of the gRPC request.
const TOKEN_PREFIX: &str = "Bearer ";
