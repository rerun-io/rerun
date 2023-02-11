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

use re_log_types::LogMsg;

pub type Result<T> = anyhow::Result<T>;

pub const PROTOCOL_VERSION: u16 = 0;

pub const DEFAULT_SERVER_PORT: u16 = 9876;

/// The default address of a Rerun TCP server which an SDK connects to.
pub fn default_server_addr() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, 1], DEFAULT_SERVER_PORT))
}

const PREFIX: [u8; 4] = *b"RR00";

pub fn encode_log_msg(log_msg: &LogMsg) -> Vec<u8> {
    use bincode::Options as _;
    let mut bytes = PREFIX.to_vec();
    bincode::DefaultOptions::new()
        .serialize_into(&mut bytes, log_msg)
        .unwrap();
    bytes
}

pub fn decode_log_msg(data: &[u8]) -> Result<LogMsg> {
    let payload = data
        .strip_prefix(&PREFIX)
        .ok_or_else(|| anyhow::format_err!("Message didn't start with the correct prefix"))?;

    use anyhow::Context as _;
    use bincode::Options as _;
    bincode::DefaultOptions::new()
        .deserialize(payload)
        .context("bincode")
}
