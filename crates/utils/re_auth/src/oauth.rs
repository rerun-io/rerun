use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::Jwt;

pub mod api;
mod storage;

/// Tokens with fewer than this number of seconds left before expiration
/// are considered expired. This ensures tokens don't become expired
/// during network transit.
const SOFT_EXPIRE_SECS: i64 = 60;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) static OAUTH_CLIENT_ID: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    std::env::var("RERUN_OAUTH_CLIENT_ID")
        .ok()
        .unwrap_or_else(|| "client_01JZ3JVR1PEVQMS73V86MC4CE2".into())
});

#[cfg(not(target_arch = "wasm32"))]
pub(crate) static OAUTH_ISSUER_URL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    std::env::var("RERUN_OAUTH_ISSUER_URL")
        .ok()
        .unwrap_or_else(|| {
            format!(
                "https://api.workos.com/user_management/{}",
                *OAUTH_CLIENT_ID
            )
        })
});

#[derive(Debug, thiserror::Error)]
#[error("failed to load credentials: {0}")]
pub struct CredentialsLoadError(#[from] storage::LoadError);

/// Load credentials from storage.
pub fn load_credentials() -> Result<Option<Credentials>, CredentialsLoadError> {
    if let Some(credentials) = storage::load()? {
        re_log::debug!("found credentials");
        Ok(Some(credentials))
    } else {
        re_log::debug!("no credentials stored locally");
        Ok(None)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialsRefreshError {
    #[error("failed to refresh credentials: {0}")]
    Api(#[from] api::Error),

    #[error("failed to store credentials: {0}")]
    Store(#[from] storage::StoreError),

    #[error("failed to deserialize credentials: {0}")]
    MalformedToken(#[from] MalformedTokenError),
}

/// Refresh credentials if they are expired.
pub async fn refresh_credentials(
    credentials: Credentials,
) -> Result<Credentials, CredentialsRefreshError> {
    // Don't refresh unless the access token has expired
    if !credentials.access_token().is_expired() {
        re_log::debug!(
            "skipping credentials refresh: credentials expire in {} seconds",
            credentials.access_token().remaining_duration_secs()
        );
        return Ok(credentials);
    }

    re_log::debug!(
        "expired {} seconds ago",
        -credentials.access_token().remaining_duration_secs()
    );

    let response = api::refresh(&credentials.refresh_token).await?;
    let credentials = Credentials::from_auth_response(response)?;
    let credentials = credentials
        .ensure_stored()
        .map_err(|err| CredentialsRefreshError::Store(err.0))?;
    re_log::debug!("credentials refreshed successfully");
    Ok(credentials)
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialsError {
    #[error("failed to load credentials: {0}")]
    Load(#[from] CredentialsLoadError),

    #[error("failed to refresh credentials: {0}")]
    Refresh(#[from] CredentialsRefreshError),
}

/// Load and immediately refresh credentials, if needed.
pub async fn load_and_refresh_credentials() -> Result<Option<Credentials>, CredentialsError> {
    match load_credentials()? {
        Some(credentials) => Ok(refresh_credentials(credentials).await.map(Some)?),
        None => Ok(None),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchJwksError {
    #[error("{0}")]
    Request(String),

    #[error("failed to deserialize JWKS: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Rerun Cloud permissions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    /// User can read data.
    #[serde(rename = "read")]
    Read,

    /// User can both read and write data.
    #[serde(rename = "read-write")]
    ReadWrite,

    #[serde(untagged)]
    Unknown(String),
}

#[allow(clippy::allow_attributes, dead_code)] // fields may become used at some point in the near future
#[derive(Debug, Serialize, Deserialize)]
pub struct RerunCloudClaims {
    /// Issuer
    pub iss: String,

    /// Subject
    pub sub: String,

    /// Actor
    pub act: Option<Act>,

    /// Organization ID
    pub org_id: String,

    pub permissions: Vec<Permission>,

    pub entitlements: Option<Vec<String>>,

    /// Session ID
    pub sid: String,

    /// Token ID
    pub jti: String,

    /// Expires at
    pub exp: i64,

    /// Issued at
    pub iat: i64,
}

impl RerunCloudClaims {
    pub const REQUIRED: &'static [&'static str] =
        &["iss", "sub", "org_id", "permissions", "exp", "iat"];
}

#[allow(clippy::allow_attributes, dead_code)] // fields may become used at some point in the near future
#[derive(Debug, Serialize, Deserialize)]
pub struct Act {
    sub: String,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum VerifyError {
    #[error("invalid jwt: {0}")]
    InvalidJwt(#[from] jsonwebtoken::errors::Error),

    #[error("missing `kid` in JWT")]
    MissingKeyId,

    #[error("key with id {id:?} was not found in JWKS")]
    KeyNotFound { id: String },
}

/// In-memory credential storage
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Credentials {
    user: User,
    refresh_token: RefreshToken,
    access_token: AccessToken,
}

pub(crate) struct InMemoryCredentials(Credentials);

#[derive(Debug, thiserror::Error)]
#[error("failed to store credentials: {0}")]
pub struct CredentialsStoreError(#[from] storage::StoreError);

impl InMemoryCredentials {
    /// Ensure credentials are persisted to disk before using them.
    pub fn ensure_stored(self) -> Result<Credentials, CredentialsStoreError> {
        storage::store(&self.0)?;
        Ok(self.0)
    }
}

impl Credentials {
    /// Deserializes credentials from an authentication response.
    ///
    /// Assumes the credentials are valid and not expired.
    pub(crate) fn from_auth_response(
        res: api::AuthenticationResponse,
    ) -> Result<InMemoryCredentials, MalformedTokenError> {
        // SAFETY: The token comes from a trusted source, which is the authentication API.
        #[expect(unsafe_code)]
        let access_token = unsafe { AccessToken::unverified(Jwt(res.access_token))? };
        Ok(InMemoryCredentials(Self {
            user: res.user,
            refresh_token: RefreshToken(res.refresh_token),
            access_token,
        }))
    }

    pub fn access_token(&self) -> &AccessToken {
        &self.access_token
    }

    /// The currently authenticated user.
    pub fn user(&self) -> &User {
        &self.user
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub metadata: HashMap<String, String>,
}

/// An access token which was valid at some point in the past.
///
/// Every time it's used, you should first check if it's expired, and refresh it if so.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct AccessToken {
    token: String,
    expires_at: i64,
}

impl AccessToken {
    pub fn jwt(&self) -> Jwt {
        Jwt(self.token.clone())
    }

    pub fn as_str(&self) -> &str {
        &self.token
    }

    pub fn is_expired(&self) -> bool {
        self.remaining_duration_secs() <= SOFT_EXPIRE_SECS
    }

    pub fn remaining_duration_secs(&self) -> i64 {
        use saturating_cast::SaturatingCast as _;

        // Time in seconds since unix epoch
        let now: i64 = jsonwebtoken::get_current_timestamp().saturating_cast();
        self.expires_at - now
    }

    /// Construct an [`AccessToken`] without verifying it.
    ///
    /// ## Safety
    ///
    /// - The token should come from a trusted source, like the Rerun auth API.
    // Note: Misusing this will not cause UB, but we're still marking it unsafe
    // to ensure it is not used lightly.
    #[expect(unsafe_code)]
    pub(crate) unsafe fn unverified(jwt: Jwt) -> Result<Self, MalformedTokenError> {
        use base64::prelude::*;

        let (_header, rest) = jwt
            .as_str()
            .split_once('.')
            .ok_or(MalformedTokenError::MissingHeaderPayloadSeparator)?;
        let (payload, _signature) = rest
            .split_once('.')
            .ok_or(MalformedTokenError::MissingPayloadSignatureSeparator)?;
        let payload = BASE64_URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(MalformedTokenError::Base64)?;
        let payload: RerunCloudClaims =
            serde_json::from_slice(&payload).map_err(MalformedTokenError::Serde)?;

        Ok(Self {
            token: jwt.0,
            expires_at: payload.exp,
        })
    }
}

impl std::fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessToken")
            .field("token", &"…")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MalformedTokenError {
    #[error("missing `.` separator between header and payload")]
    MissingHeaderPayloadSeparator,

    #[error("missing `.` separator between payload and signature")]
    MissingPayloadSignatureSeparator,

    #[error("failed to decode base64 payload: {0}")]
    Base64(base64::DecodeError),

    #[error("failed to deserialize payload: {0}")]
    Serde(serde_json::Error),
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub(crate) struct RefreshToken(String);

impl std::fmt::Debug for RefreshToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RefreshToken").field(&"…").finish()
    }
}
