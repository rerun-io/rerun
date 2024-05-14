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
pub use {buffered_client::Client, tcp_client::ClientError};

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
pub use server::{serve, ConnectionError, ServerError, ServerOptions};

pub const PROTOCOL_VERSION_0: u16 = 0;

/// Added [`PROTOCOL_HEADER`]. Introduced for Rerun 0.16.
pub const PROTOCOL_VERSION_1: u16 = 1;

/// Comes after version.
pub const PROTOCOL_HEADER: &str = "rerun";

pub const DEFAULT_SERVER_PORT: u16 = 9876;

/// The default address of a Rerun TCP server which an SDK connects to.
pub fn default_server_addr() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, 1], DEFAULT_SERVER_PORT))
}

/// The default amount of time to wait for the TCP connection to resume during a flush
#[allow(clippy::unnecessary_wraps)]
pub fn default_flush_timeout() -> Option<std::time::Duration> {
    Some(std::time::Duration::from_secs(2))
}
