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

    #[error("token verification failed")]
    InvalidToken,

    #[cfg(feature = "workos")]
    #[error("failed to fetch JWKS from WorkOS")]
    JwksFetch(crate::workos::api::Error),

    #[cfg(feature = "workos")]
    #[error(
        "no external provider configured, configure one using `RedapProvider::with_external_provider`"
    )]
    NoExternalProvider,
}
