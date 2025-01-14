/// Handles errors for the `re_auth` crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transparent")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("transparent")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("transparent")]
    SystemTime(#[from] std::time::SystemTimeError),

    #[error("failed to parse token")]
    MalformedToken,
}
