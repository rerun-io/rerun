//! A Rerun server implementation backed by an in-memory store.

mod entrypoint;
mod rerun_cloud;
mod server;
mod store;

#[cfg(feature = "lance")]
mod chunk_index;

pub use self::{
    entrypoint::{Args, NamedPath},
    rerun_cloud::{RerunCloudHandler, RerunCloudHandlerBuilder, RerunCloudHandlerSettings},
    server::{Server, ServerBuilder, ServerError, ServerHandle},
};

/// What should we do on error?
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OnError {
    Continue,
    Abort,
}
