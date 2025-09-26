//! Official gRPC client for the Rerun Data Protocol.

mod connection_client;
mod connection_registry;
mod grpc;

pub use self::{
    connection_client::GenericConnectionClient,
    connection_registry::{
        ClientConnectionError, ConnectionClient, ConnectionRegistry, ConnectionRegistryHandle,
    },
    grpc::{
        ConnectionError, RedapClient, UiCommand, channel,
        fetch_chunks_response_to_chunk_and_partition_id,
        get_chunks_response_to_chunk_and_partition_id, stream_blueprint_and_partition_from_server,
        stream_dataset_from_redap,
    },
};

const MAX_DECODING_MESSAGE_SIZE: usize = u32::MAX as usize;

/// Wrapper with a nicer error message
#[derive(Debug)]
pub struct TonicStatusError(Box<tonic::Status>);

const _: () = assert!(
    std::mem::size_of::<TonicStatusError>() <= 32,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

impl AsRef<tonic::Status> for TonicStatusError {
    #[inline]
    fn as_ref(&self) -> &tonic::Status {
        &self.0
    }
}

impl TonicStatusError {
    /// Returns the inner [`tonic::Status`].
    pub fn into_inner(self) -> tonic::Status {
        *self.0
    }
}

impl std::fmt::Display for TonicStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO(emilk): duplicated in `re_grpc_server`
        let status = &self.0;

        write!(f, "gRPC error")?;

        if status.code() != tonic::Code::Unknown {
            write!(f, ", code: '{}'", status.code())?;
        }
        if !status.message().is_empty() {
            write!(f, ", message: {:?}", status.message())?;
        }
        // Binary data - not useful.
        // if !status.details().is_empty() {
        //     write!(f, ", details: {:?}", status.details())?;
        // }
        if !status.metadata().is_empty() {
            write!(f, ", metadata: {:?}", status.metadata().as_ref())?;
        }
        Ok(())
    }
}

impl From<tonic::Status> for TonicStatusError {
    fn from(value: tonic::Status) -> Self {
        Self(Box::new(value))
    }
}

impl std::error::Error for TonicStatusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StreamEntryError {
    #[error("Failed reading entry\nDetails:{0}")]
    Read(TonicStatusError),

    #[error("Failed finding entry\nDetails:{0}")]
    Find(TonicStatusError),

    #[error("Failed deleting entry\nDetails:{0}")]
    Delete(TonicStatusError),

    #[error("Failed updating entry\nDetails:{0}")]
    Update(TonicStatusError),

    #[error("Failed creating entry\nDetails:{0}")]
    Create(TonicStatusError),

    #[error("Failed reading entry's partitions\nDetails:{0}")]
    ReadPartitions(TonicStatusError),

    #[error("Failed registering data source with entry\nDetails:{0}")]
    RegisterData(TonicStatusError),

    #[error("Failed registering table\nDetails:{0}")]
    RegisterTable(TonicStatusError),

    #[error("Error while doing maintenance on entry\nDetails:{0}")]
    Maintenance(TonicStatusError),

    #[error("Invalid entry id\nDetails:{0}")]
    InvalidId(TonicStatusError),
}

#[derive(thiserror::Error, Debug)]
pub enum StreamPartitionError {
    #[error("Failed streaming partition chunks\nDetails:{0}")]
    StreamingChunks(TonicStatusError),
}

#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    #[error(transparent)]
    ClientConnectionError(#[from] ClientConnectionError),

    #[error(transparent)]
    EntryError(#[from] StreamEntryError),

    #[error(transparent)]
    PartitionError(#[from] StreamPartitionError),

    #[error(transparent)]
    Tokio(#[from] tokio::task::JoinError),

    #[error(transparent)]
    CodecError(#[from] re_log_encoding::codec::CodecError),

    #[error(transparent)]
    ChunkError(#[from] re_chunk::ChunkError),

    #[error(transparent)]
    DecodeError(#[from] re_log_encoding::decoder::DecodeError),

    #[error(transparent)]
    TypeConversionError(#[from] re_protos::TypeConversionError),

    #[error("Column '{0}' is missing from the dataframe")]
    MissingDataframeColumn(String),

    #[error("{0}")]
    MissingData(String),

    #[error("arrow error: {0}")]
    ArrowError(#[from] arrow::error::ArrowError),
}

const _: () = assert!(
    std::mem::size_of::<StreamError>() <= 80,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

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
