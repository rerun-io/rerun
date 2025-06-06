//! Communications with an Rerun Data Platform gRPC server.

mod connection_registry;
pub mod message_proxy;
mod redap;

pub use self::{
    connection_registry::{ConnectionRegistry, ConnectionRegistryHandle},
    redap::{
        Command, ConnectionError, RedapClient, channel,
        get_chunks_response_to_chunk_and_partition_id, stream_blueprint_and_partition_from_server,
        stream_dataset_from_redap,
    },
};

const MAX_DECODING_MESSAGE_SIZE: usize = u32::MAX as usize;

/// Wrapper with a nicer error message
#[derive(Debug)]
pub struct TonicStatusError(pub tonic::Status);

impl std::fmt::Display for TonicStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = &self.0;
        write!(f, "gRPC error, status: '{}'", status.code())?;
        if !status.message().is_empty() {
            write!(f, ", message: {:?}", status.message())?;
        }
        // Binary data - not useful.
        // if !status.details().is_empty() {
        //     write!(f, ", details: {:?}", status.details())?;
        // }
        if !status.metadata().is_empty() {
            write!(f, ", metadata: {:?}", status.metadata())?;
        }
        Ok(())
    }
}

impl From<tonic::Status> for TonicStatusError {
    fn from(value: tonic::Status) -> Self {
        Self(value)
    }
}

impl std::error::Error for TonicStatusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error("connection failed: {0}")]
    Transport(#[from] tonic::transport::Error),

    #[error(transparent)]
    ConnectionError(#[from] redap::ConnectionError),

    #[error(transparent)]
    TonicStatus(#[from] TonicStatusError),

    #[error(transparent)]
    CodecError(#[from] re_log_encoding::codec::CodecError),

    #[error(transparent)]
    ChunkError(#[from] re_chunk::ChunkError),

    #[error(transparent)]
    DecodeError(#[from] re_log_encoding::decoder::DecodeError),

    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    #[error(transparent)]
    InvalidSorbetSchema(#[from] re_sorbet::SorbetError),

    #[error(transparent)]
    TypeConversionError(#[from] re_protos::TypeConversionError),

    #[error("Chunk data missing in response")]
    MissingChunkData,
}

impl From<tonic::Status> for StreamError {
    fn from(value: tonic::Status) -> Self {
        Self::TonicStatus(value.into())
    }
}

// TODO(ab, andreas): This should be replaced by the use of `AsyncRuntimeHandle`. However, this
// requires:
// - `AsyncRuntimeHandle` to be moved lower in the crate hierarchy to be available here (unsure
//   where).
// - Make sure that all callers of `DataSource::stream` have access to an `AsyncRuntimeHandle`
//   (maybe it should be in `GlobalContext`?).
#[cfg(target_arch = "wasm32")]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static + Send,
{
    tokio::spawn(future);
}
