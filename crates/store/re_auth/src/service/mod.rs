//! This module contains auth middleware for [`tonic`] services.

pub use server::AuthInterceptor;

mod client;
mod server;

/// The metadata key used in the metadata of the gRPC request to store the token.
const AUTHORIZATION_KEY: &str = "authorization";

/// The prefix for the token in the metadata of the gRPC request.
const TOKEN_PREFIX: &str = "Bearer ";
