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

#![warn(missing_docs)] // Let's keep the this crate well-documented!

// ----------------
// Private modules:

mod binary_stream_sink;
mod global;
mod log_sink;
mod recording_stream;
mod spawn;

// ---------------
// Public modules:

pub mod blueprint;

// -------------
// Public items:

pub use spawn::{SpawnError, SpawnOptions, spawn};

pub use self::recording_stream::{
    RecordingStream, RecordingStreamBuilder, RecordingStreamError, RecordingStreamResult,
    forced_sink_path,
};

/// The default port of a Rerun gRPC /proxy server.
pub const DEFAULT_SERVER_PORT: u16 = re_uri::DEFAULT_PROXY_PORT;

/// The default URL of a Rerun gRPC /proxy server.
///
/// This isn't used to _host_ the server, only to _connect_ to it.
pub const DEFAULT_CONNECT_URL: &str =
    const_format::concatcp!("rerun+http://127.0.0.1:", DEFAULT_SERVER_PORT, "/proxy");

pub use global::cleanup_if_forked_child;
pub use re_log_types::{
    ApplicationId, EntityPath, EntityPathFilter, EntityPathPart, Instance, StoreId, StoreKind,
    entity_path,
};
pub use re_sdk_types::archetypes::RecordingInfo;

#[cfg(not(target_arch = "wasm32"))]
impl crate::sink::LogSink for re_log_encoding::FileSink {
    fn send(&self, msg: re_log_types::LogMsg) {
        Self::send(self, msg);
    }

    #[inline]
    fn flush_blocking(&self, timeout: std::time::Duration) -> Result<(), sink::SinkFlushError> {
        use re_log_encoding::FileFlushError;

        Self::flush_blocking(self, timeout).map_err(|err| match err {
            FileFlushError::Failed { message } => sink::SinkFlushError::Failed { message },
            FileFlushError::Timeout => sink::SinkFlushError::Timeout,
        })
    }
}

// ---------------
// Public modules:

/// Different destinations for log messages.
///
/// This is how you select whether the log stream ends up
/// sent over gRPC, written to file, etc.
pub mod sink {
    #[cfg(not(target_arch = "wasm32"))]
    pub use re_log_encoding::{FileSink, FileSinkError};

    pub use crate::binary_stream_sink::{BinaryStreamSink, BinaryStreamStorage};
    pub use crate::log_sink::{
        BufferedSink, CallbackSink, GrpcSink, GrpcSinkConnectionFailure, GrpcSinkConnectionState,
        IntoMultiSink, LogSink, MemorySink, MemorySinkStorage, MultiSink, SinkFlushError,
    };
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

pub use re_sdk_types::{
    Archetype, ArchetypeName, AsComponents, Component, ComponentBatch, ComponentDescriptor,
    ComponentIdentifier, ComponentType, DatatypeName, DeserializationError, DeserializationResult,
    Loggable, SerializationError, SerializationResult, SerializedComponentBatch,
    SerializedComponentColumn,
};
pub use time::{TimeCell, TimePoint, Timeline};

/// Transformation and reinterpretation of components.
///
/// # Experimental
///
/// This is an experimental API and may change in future releases.
pub mod lenses;

pub use re_byte_size::SizeBytes;
#[cfg(feature = "data_loaders")]
pub use re_data_loader::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

/// Methods for spawning the web viewer and streaming the SDK log stream to it.
#[cfg(feature = "web_viewer")]
pub mod web_viewer;

/// Method for spawning a gRPC server and streaming the SDK log stream to it.
#[cfg(feature = "server")]
pub mod grpc_server;

#[cfg(feature = "server")]
pub use re_grpc_server::{MemoryLimit, PlaybackBehavior, ServerOptions};

/// Re-exports of other crates.
pub mod external {
    pub use re_chunk::external::*;
    #[cfg(feature = "data_loaders")]
    pub use re_data_loader::{self, external::*};
    #[cfg(feature = "server")]
    pub use re_grpc_server;
    pub use re_log::external::*;
    pub use re_log_types::external::*;
    pub use {re_grpc_client, re_log, re_log_encoding, re_log_types, re_uri};
}

#[cfg(feature = "web_viewer")]
pub use web_viewer::serve_web_viewer;

// -----
// Misc:

/// The version of the Rerun SDK.
pub fn build_info() -> re_build_info::BuildInfo {
    re_build_info::build_info!()
}

const RERUN_ENV_VAR: &str = "RERUN";

/// Helper to get the value of the `RERUN` environment variable.
fn get_rerun_env() -> Option<bool> {
    let s = std::env::var(RERUN_ENV_VAR).ok()?;
    match s.to_lowercase().as_str() {
        "0" | "false" | "off" => Some(false),
        "1" | "true" | "on" => Some(true),
        _ => {
            re_log::warn!(
                "Invalid value for environment variable {RERUN_ENV_VAR}={s:?}. Expected 'on' or 'off'. It will be ignored"
            );
            None
        }
    }
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
    let store_id = StoreId::random(StoreKind::Recording, application_id.into());

    re_log_types::StoreInfo::new(
        store_id,
        re_log_types::StoreSource::RustSdk {
            rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
            llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
        },
    )
}
