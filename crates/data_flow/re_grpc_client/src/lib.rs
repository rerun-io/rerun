//! Client for the legacy `StoreHub` API (`re_grpc_server`).

pub mod read;
pub use read::stream;

#[cfg(not(target_arch = "wasm32"))]
pub mod write;

#[cfg(not(target_arch = "wasm32"))]
pub use write::Client;

#[cfg(not(target_arch = "wasm32"))]
pub mod write_table;

pub const MAX_DECODING_MESSAGE_SIZE: usize = u32::MAX as usize;

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
        // NOTE: duplicated in `re_grpc_server` and `re_redap_client`
        fmt_tonic_status(f, &self.0)
    }
}

fn fmt_tonic_status(f: &mut std::fmt::Formatter<'_>, status: &tonic::Status) -> std::fmt::Result {
    // The server message may come with details of its own, which must stay details:
    // the status code belongs on the summary.
    let mut error = re_error::StructuredError::parse(status.message());

    if error.summary.is_empty() {
        error.summary = "gRPC error".to_owned();
    }

    let code = status.code();
    if code != tonic::Code::Unknown {
        // The `Debug` name ("NotFound"), not tonic's long `Display` prose.
        error.summary = format!("{} ({code:?})", error.summary);
    }

    if !status.metadata().is_empty() {
        error.add_detail(format!("metadata: {:?}", status.metadata().as_ref()));
    }

    write!(f, "{error}")
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
