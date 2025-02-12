//! Communications with an Rerun Data Platform gRPC server.

pub mod message_proxy;
pub use message_proxy::MessageProxyUrl;

#[cfg(feature = "redap")]
pub mod redap;

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

impl std::error::Error for TonicStatusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),

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
}

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
