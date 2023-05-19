//! Communications between server and viewer.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#[cfg(feature = "client")]
mod client;
use std::{fmt::Display, str::FromStr};

#[cfg(feature = "client")]
pub use client::Connection;

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
pub use server::{RerunServer, RerunServerHandle};

use re_log_types::LogMsg;

pub const DEFAULT_WS_SERVER_PORT: u16 = 9877;

#[cfg(feature = "tls")]
pub const PROTOCOL: &str = "wss";

#[cfg(not(feature = "tls"))]
pub const PROTOCOL: &str = "ws";

// ----------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum RerunServerError {
    #[error("failed to bind to port {0}: {1}")]
    BindFailed(RerunServerPort, std::io::Error),

    #[error("received an invalid message")]
    InvalidMessagePrefix,

    #[error("received an invalid message")]
    InvalidMessage(#[from] bincode::Error),

    #[cfg(feature = "server")]
    #[error("failed to join web viewer server task: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[cfg(feature = "server")]
    #[error("tokio error: {0}")]
    TokioIoError(#[from] tokio::io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Typed port for use with [`RerunServer`]
pub struct RerunServerPort(pub u16);

impl Default for RerunServerPort {
    fn default() -> Self {
        Self(DEFAULT_WS_SERVER_PORT)
    }
}

impl Display for RerunServerPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Needed for clap
impl FromStr for RerunServerPort {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u16>() {
            Ok(port) => Ok(RerunServerPort(port)),
            Err(err) => Err(format!("Failed to parse port: {err}")),
        }
    }
}

/// Add a protocol (`ws://` or `wss://`) to the given address.
pub fn server_url(local_addr: &std::net::SocketAddr) -> String {
    if local_addr.ip().is_unspecified() {
        // "0.0.0.0"
        format!("{PROTOCOL}://localhost:{}", local_addr.port())
    } else {
        format!("{PROTOCOL}://{local_addr}")
    }
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

pub fn decode_log_msg(data: &[u8]) -> Result<LogMsg, RerunServerError> {
    let payload = data
        .strip_prefix(&PREFIX)
        .ok_or(RerunServerError::InvalidMessagePrefix)?;

    use bincode::Options as _;
    Ok(bincode::DefaultOptions::new().deserialize(payload)?)
}
