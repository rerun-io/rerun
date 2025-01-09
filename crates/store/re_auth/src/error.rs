/// Handles errors for the `re_auth` crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to decode: {0}")]
    InvalidBase64(base64::DecodeError),

    #[error("failed to authenticate: {0}")]
    InvalidAuthentication(jwt_simple::Error),

    #[error("failed to parse token")]
    MalformedToken,

    #[error("failed to verify token: {0}")]
    InvalidToken(jwt_simple::Error),

    #[error("invalid permission: expected `{expected}` but got `{actual}`")]
    InvalidPermission { expected: String, actual: String },
}
