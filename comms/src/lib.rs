//! Communications between server and viewer

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use client::Connection;

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
pub use server::Server;

use log_types::DataMsg;

pub type Result<T> = anyhow::Result<T>;

pub const DEFAULT_SERVER_PORT: u16 = 9876;

#[cfg(feature = "tls")]
pub const PROTOCOL: &str = "wss";

#[cfg(not(feature = "tls"))]
pub const PROTOCOL: &str = "ws";

pub fn default_server_url() -> String {
    format!("{PROTOCOL}://127.0.0.1:{DEFAULT_SERVER_PORT}")
}

const PREFIX: [u8; 4] = *b"RR00";

pub fn encode_log_msg(data_msg: &DataMsg) -> Vec<u8> {
    use bincode::Options as _;
    let mut bytes = PREFIX.to_vec();
    bincode::DefaultOptions::new()
        .serialize_into(&mut bytes, data_msg)
        .unwrap();
    bytes
}

pub fn decode_log_msg(data: &[u8]) -> Result<DataMsg> {
    let payload = data
        .strip_prefix(&PREFIX)
        .ok_or_else(|| anyhow::format_err!("Message didn't start with the correct prefix"))?;

    use anyhow::Context as _;
    use bincode::Options as _;
    bincode::DefaultOptions::new()
        .deserialize(payload)
        .context("bincode")
}
