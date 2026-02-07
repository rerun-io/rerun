//! A Rerun server implementation backed by an in-memory store.

#[cfg(feature = "lance")]
mod chunk_index;

mod entrypoint;
mod latency_layer;
mod rerun_cloud;
mod server;
mod store;

pub use self::entrypoint::{Args, NamedPath, NamedPathCollection};
pub use self::rerun_cloud::{
    RerunCloudHandler, RerunCloudHandlerBuilder, RerunCloudHandlerSettings,
};
pub use self::server::{Server, ServerBuilder, ServerError, ServerHandle};

/// What should we do on error?
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OnError {
    Continue,
    Abort,
}
