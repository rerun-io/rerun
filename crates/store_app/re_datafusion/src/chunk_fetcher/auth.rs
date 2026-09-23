use std::error::Error;

/// Something that authenticates object store direct fetch requests.
pub trait ObjectStoreAuthenticator: std::fmt::Debug + Send + Sync + 'static {
    /// Add authentication to a batch of requests.
    ///
    /// This is batched for performance reasons, as some implementations might need to perform
    /// expensive work per each call, e.g. cross boundaries into a Python call in the SDK.
    fn authenticate_requests(
        &self,
        requests: &mut dyn Iterator<Item = &mut reqwest::Request>,
    ) -> Result<(), Box<dyn Error>>;

    /// Whether requests to the server should ask for URLs to be signed.
    ///
    /// If returning `false` (the default), this means the server will respond with a plain HTTPS URL
    /// and the client is responsible for providing authentication to the request.
    /// If returning `true`, the server will be asked to return a presigned URL that can be used as-is.
    fn needs_signed_urls(&self) -> bool {
        false
    }
}

/// An authenticator that does nothing, and asks for object URLs to be signed by the server.
#[derive(Default, Clone, Copy, Debug)]
pub struct NoOpObjectStoreAuthenticator {}

impl ObjectStoreAuthenticator for NoOpObjectStoreAuthenticator {
    fn authenticate_requests(
        &self,
        _requests: &mut dyn Iterator<Item = &mut reqwest::Request>,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn needs_signed_urls(&self) -> bool {
        true
    }
}
