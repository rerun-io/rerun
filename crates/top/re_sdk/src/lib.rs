//! The Rerun logging SDK
//!
//! This is the bare-bones version of the [`rerun`](https://docs.rs/rerun/) crate.
//! `rerun` exports everything in `re_sdk`, so in most cases you want to use `rerun`
//! instead.
//!
//! Please read [the docs for the `rerun` crate](https://docs.rs/rerun/) instead.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// TODO(#6330): remove unwrap()
#![allow(clippy::unwrap_used)]
#![warn(missing_docs)] // Let's keep the this crate well-documented!

// ----------------
// Private modules:

mod binary_stream_sink;
mod client;
mod global;
mod log_sink;
mod recording_stream;
mod spawn;

// -------------
// Public items:

pub use spawn::{spawn, SpawnError, SpawnOptions};

pub use self::client::RerunClient;
pub use self::recording_stream::{
    forced_sink_path, RecordingStream, RecordingStreamBuilder, RecordingStreamError,
    RecordingStreamResult,
};

/// The default port of a Rerun gRPC server.
pub const DEFAULT_SERVER_PORT: u16 = 9876;

/// The default URL of a Rerun gRPC server.
///
/// This isn't used to _host_ the server, only to _connect_ to it.
pub const DEFAULT_CONNECT_URL: &str =
    const_format::concatcp!("rerun+http://127.0.0.1:", DEFAULT_SERVER_PORT, "/proxy");

/// The default address of a Rerun gRPC server which an SDK connects to.
#[deprecated(since = "0.22.0", note = "migrate to connect_grpc")]
pub fn default_server_addr() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, 1], DEFAULT_SERVER_PORT))
}

/// The default amount of time to wait for the gRPC connection to resume during a flush
#[allow(clippy::unnecessary_wraps)]
pub fn default_flush_timeout() -> Option<std::time::Duration> {
    // NOTE: This is part of the SDK and meant to be used where we accept `Option<std::time::Duration>` values.
    Some(std::time::Duration::from_secs(2))
}

pub use re_log_types::{
    entity_path, ApplicationId, EntityPath, EntityPathPart, Instance, StoreId, StoreKind,
};
pub use re_memory::MemoryLimit;
pub use re_types::archetypes::RecordingProperties;

pub use global::cleanup_if_forked_child;

#[cfg(not(target_arch = "wasm32"))]
impl crate::sink::LogSink for re_log_encoding::FileSink {
    fn send(&self, msg: re_log_types::LogMsg) {
        Self::send(self, msg);
    }

    #[inline]
    fn flush_blocking(&self) {
        Self::flush_blocking(self);
    }
}

// ---------------
// Public modules:

/// Different destinations for log messages.
///
/// This is how you select whether the log stream ends up
/// sent over gRPC, written to file, etc.
pub mod sink {
    pub use crate::binary_stream_sink::{BinaryStreamSink, BinaryStreamStorage};
    pub use crate::log_sink::{BufferedSink, CallbackSink, LogSink, MemorySink, MemorySinkStorage};

    pub use crate::log_sink::GrpcSink;

    #[cfg(not(target_arch = "wasm32"))]
    pub use re_log_encoding::{FileSink, FileSinkError};
}

/// Things directly related to logging.
pub mod log {
    pub use re_chunk::{
        Chunk, ChunkBatcher, ChunkBatcherConfig, ChunkBatcherError, ChunkBatcherResult,
        ChunkComponents, ChunkError, ChunkId, ChunkResult, PendingRow, RowId, TimeColumn,
    };
    pub use re_log_types::LogMsg;
}

/// Time-related types.
pub mod time {
    pub use re_log_types::{Duration, TimeCell, TimeInt, TimePoint, TimeType, Timeline, Timestamp};
}
pub use time::{TimeCell, TimePoint, Timeline};

pub use re_types::{
    Archetype, ArchetypeName, AsComponents, Component, ComponentBatch, ComponentDescriptor,
    ComponentName, DatatypeName, DeserializationError, DeserializationResult,
    GenericIndicatorComponent, Loggable, LoggableBatch, NamedIndicatorComponent,
    SerializationError, SerializationResult, SerializedComponentBatch, SerializedComponentColumn,
};

pub use re_byte_size::SizeBytes;

#[cfg(feature = "data_loaders")]
pub use re_data_loader::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

/// Methods for spawning the web viewer and streaming the SDK log stream to it.
#[cfg(feature = "web_viewer")]
pub mod web_viewer;

/// Method for spawning a gRPC server and streaming the SDK log stream to it.
#[cfg(feature = "server")]
pub mod grpc_server;

/// Re-exports of other crates.
pub mod external {
    pub use re_grpc_client;
    pub use re_grpc_server;
    pub use re_log;
    pub use re_log_encoding;
    pub use re_log_types;

    pub use re_chunk::external::*;
    pub use re_log::external::*;
    pub use re_log_types::external::*;

    #[cfg(feature = "data_loaders")]
    pub use re_data_loader;
}

// -----
// Misc:

/// The version of the Rerun SDK.
pub fn build_info() -> re_build_info::BuildInfo {
    re_build_info::build_info!()
}

const RERUN_ENV_VAR: &str = "RERUN";

/// Helper to get the value of the `RERUN` environment variable.
fn get_rerun_env() -> Option<bool> {
    std::env::var(RERUN_ENV_VAR)
        .ok()
        .and_then(|s| match s.to_lowercase().as_str() {
            "0" | "false" | "off" => Some(false),
            "1" | "true" | "on" => Some(true),
            _ => {
                re_log::warn!(
                    "Invalid value for environment variable {RERUN_ENV_VAR}={s:?}. Expected 'on' or 'off'. It will be ignored"
                );
                None
            }
        })
}

/// Checks the `RERUN` environment variable. If not found, returns the argument.
///
/// Also adds some helpful logging.
pub fn decide_logging_enabled(default_enabled: bool) -> bool {
    // We use `info_once` so that we can call this function
    // multiple times without spamming the log.
    match get_rerun_env() {
        Some(true) => {
            re_log::info_once!(
                "Rerun Logging is enabled by the '{RERUN_ENV_VAR}' environment variable."
            );
            true
        }
        Some(false) => {
            re_log::info_once!(
                "Rerun Logging is disabled by the '{RERUN_ENV_VAR}' environment variable."
            );
            false
        }
        None => {
            if !default_enabled {
                re_log::info_once!(
                    "Rerun Logging has been disabled. Turn it on with the '{RERUN_ENV_VAR}' environment variable."
                );
            }
            default_enabled
        }
    }
}

// ----------------------------------------------------------------------------

/// Creates a new [`re_log_types::StoreInfo`] which can be used with [`RecordingStream::new`].
#[track_caller] // track_caller so that we can see if we are being called from an official example.
pub fn new_store_info(
    application_id: impl Into<re_log_types::ApplicationId>,
) -> re_log_types::StoreInfo {
    re_log_types::StoreInfo {
        application_id: application_id.into(),
        store_id: StoreId::random(StoreKind::Recording),
        cloned_from: None,
        store_source: re_log_types::StoreSource::RustSdk {
            rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
            llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
        },
        store_version: Some(re_build_info::CrateVersion::LOCAL),
    }
}
