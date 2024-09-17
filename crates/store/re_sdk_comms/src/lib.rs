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
pub use server::{serve, ServerError, ServerOptions};

/// Server connection error.
///
/// This can only occur when using the `server` feature,
/// However it is defined here so that crates that want to react to this error can do so without
/// needing to depend on the `server` feature directly.
/// This is useful when processing errors from a passed-in `re_smart_channel` channel as done by `re_viewer` as of writing.
#[derive(thiserror::Error, Debug)]
pub enum ConnectionError {
    #[error("An unknown client tried to connect")]
    UnknownClient,

    #[error(transparent)]
    VersionError(#[from] VersionError),

    #[error(transparent)]
    SendError(#[from] std::io::Error),

    #[error(transparent)]
    #[cfg(feature = "server")]
    DecodeError(#[from] re_log_encoding::decoder::DecodeError),

    #[error("The receiving end of the channel was closed")]
    ChannelDisconnected(#[from] re_smart_channel::SendError<re_log_types::LogMsg>),
}

#[derive(thiserror::Error, Debug)]
#[allow(unused)]
pub enum VersionError {
    #[error("SDK client is using an older protocol version ({client_version}) than the SDK server ({server_version})")]
    ClientIsOlder {
        client_version: u16,
        server_version: u16,
    },

    #[error("SDK client is using a newer protocol version ({client_version}) than the SDK server ({server_version})")]
    ClientIsNewer {
        client_version: u16,
        server_version: u16,
    },
}

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
    // NOTE: This is part of the SDK and meant to be used where we accept `Option<std::time::Duration>` values.
    Some(std::time::Duration::from_secs(2))
}
