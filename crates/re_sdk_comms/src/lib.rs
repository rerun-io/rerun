//! TCP communications between a Rerun logging SDK and server/viewer.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#[cfg(feature = "client")]
pub(crate) mod tcp_client;

#[cfg(feature = "client")]
mod buffered_client;

#[cfg(feature = "client")]
pub use buffered_client::Client;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
pub use server::{serve, ServerOptions};

pub type Result<T> = anyhow::Result<T>;

pub const PROTOCOL_VERSION: u16 = 0;

pub const DEFAULT_SERVER_PORT: u16 = 9876;

/// The default address of a Rerun TCP server which an SDK connects to.
pub fn default_server_addr() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, 1], DEFAULT_SERVER_PORT))
}
