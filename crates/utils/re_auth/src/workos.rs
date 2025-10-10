use std::collections::HashMap;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use saturating_cast::SaturatingCast as _;
use serde::{Deserialize, Serialize};

use crate::Jwt;

pub mod api;
mod storage;

/// Tokens with fewer than this number of seconds left before expiration
/// are considered expired. This ensures tokens don't become expired
/// during network transit.
const SOFT_EXPIRE_SECS: i64 = 60;

// TODO: implement client-side credential providers
// - StaticCredentialProvider: `REDAP_TOKEN` env or `fallback_token`
// - OauthCredentialProvider: Uses same strategy as CLI
//   - Read credentials from disk
//   - Refresh if necessary
//
// TODO: stronger consistency guarantees
// - When storing credentials for refresh on native, lock the file

const ISSUER_URL_BASE: &str = "https://api.workos.com/user_management";

// TODO(jan): This is the client ID for the WorkOS public staging environment
// should be replaced by our actual client ID at some point.
// When doing so, don't forget to replace it everywhere. :)
pub(crate) const WORKOS_CLIENT_ID: &str = match option_env!("WORKOS_CLIENT_ID") {
    Some(v) => v,
    None => "client_01JZ3JVQW6JNVXME6HV9G4VR0H",
};
pub(crate) const DEFAULT_ISSUER: &str =
    const_format::concatcp!(ISSUER_URL_BASE, "/", WORKOS_CLIENT_ID);

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

    #[error("failed to deserialize credentials from WorkOS: {0}")]
    MalformedToken(#[from] MalformedTokenError),
}

/// Refresh credentials if they are expired.
pub async fn refresh_credentials(
    credentials: Credentials,
) -> Result<Credentials, CredentialsRefreshError> {
    eprintln!("{credentials:?}");

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

/// Verify a JWT's claims.
///
/// Returns `Ok(None)` if the token is expired.
pub fn verify_token(jwks: &JwkSet, jwt: Jwt) -> Result<Option<AccessToken>, VerifyError> {
    // 1. decode header to get `kid`
    let header = decode_header(jwt.as_str())?;
    let kid = header.kid.as_ref().ok_or(VerifyError::MissingKeyId)?;

    // 2. find the associated JWK
    let key = jwks
        .find(kid)
        .ok_or_else(|| VerifyError::KeyNotFound { id: kid.clone() })?;
    let key = DecodingKey::from_jwk(key)?;

    // 3. verify token claims
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[DEFAULT_ISSUER]);
    validation.validate_exp = true;
    let token = match decode::<Claims>(jwt.as_str(), &key, &validation) {
        Ok(v) => v,
        Err(err) => match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                return Ok(None);
            }
            _ => return Err(err.into()),
        },
    };

    Ok(Some(AccessToken {
        token: jwt.0,
        expires_at: token.claims.exp,
    }))
}

#[derive(Debug, thiserror::Error)]
pub enum FetchJwksError {
    #[error("{0}")]
    Request(String),

    #[error("failed to deserialize JWKS: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Permissions defined for Redap through the `WorkOS` dashboard.
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
pub struct Claims {
    /// Issuer
    pub iss: String,

    /// Subject
    pub sub: String,

    /// Actor
    pub act: Option<Act>,

    /// Organization ID
    pub org_id: Option<String>,

    /// Role
    pub role: Option<String>,

    pub permissions: Option<Vec<Permission>>,

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
    #[allow(dead_code)] // only used on CLI path, causes warnings downstream
    /// Deserializes credentials from an authentication response.
    ///
    /// Assumes the credentials are valid and not expired.
    pub(crate) fn from_auth_response(
        res: api::AuthenticationResponse,
    ) -> Result<InMemoryCredentials, MalformedTokenError> {
        // SAFETY: The token comes from a trusted source, which is the authentication API.
        #[allow(unsafe_code)]
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
        // Time in seconds since unix epoch
        let now = jsonwebtoken::get_current_timestamp() as i64;
        self.expires_at - now
    }

    /// Construct an [`AccessToken`] without verifying it.
    ///
    /// ## Safety
    ///
    /// - The token should come from a trusted source, like the `WorkOS` API.
    // Note: This is not memory unsafe, but we're still marking it unsafe
    // to ensure it is not used lightly.
    #[allow(unsafe_code)]
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
        let payload: Claims =
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
            .field("token", &"...")
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
        f.debug_tuple("RefreshToken").field(&"...").finish()
    }
}
