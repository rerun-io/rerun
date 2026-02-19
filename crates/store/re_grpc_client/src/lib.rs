//! Client for the legacy `StoreHub` API (`re_grpc_server`).

pub mod read;
pub use read::stream;

#[cfg(not(target_arch = "wasm32"))]
pub mod write;

#[cfg(not(target_arch = "wasm32"))]
pub use write::Client;

#[cfg(not(target_arch = "wasm32"))]
pub mod write_table;

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
pub enum StreamError {
    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error("connection failed: {0}")]
    Transport(#[from] tonic::transport::Error),

    #[error(transparent)]
    TonicStatus(#[from] TonicStatusError),

    #[error(transparent)]
    Codec(#[from] re_log_encoding::rrd::CodecError),
}

const _: () = assert!(
    std::mem::size_of::<StreamError>() <= 80,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

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
//   (maybe it should be in `AppContext`?).
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
