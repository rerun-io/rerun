//! Communications between server and viewer.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use client::Connection;

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
pub use server::Server;

use re_log_types::LogMsg;

pub type Result<T> = anyhow::Result<T>;

pub const DEFAULT_WS_SERVER_PORT: u16 = 9877;

#[cfg(feature = "tls")]
pub const PROTOCOL: &str = "wss";

#[cfg(not(feature = "tls"))]
pub const PROTOCOL: &str = "ws";

pub fn default_server_url(hostname: &str) -> String {
    format!("{PROTOCOL}://{hostname}:{DEFAULT_WS_SERVER_PORT}")
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
