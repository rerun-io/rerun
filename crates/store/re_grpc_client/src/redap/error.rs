#[derive(thiserror::Error, Debug)]
pub enum ConnectionError {
    /// The given url is not a valid Rerun storage node URL.
    #[error("URL {url:?} should follow rerun://host:port/recording/12345 for recording or rerun://host:port/catalog for catalog")]
    InvalidRedapAddress {
    url: String,
    msg: String
}

    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),
}
