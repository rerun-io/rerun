use std::{collections::HashMap, sync::Arc};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde::{Deserialize, Serialize};

use crate::Jwt;

/// Tokens with less than this number of seconds left before expiration
/// are considered expired. This ensures tokens don't become expired
/// during network transit.
const SOFT_EXPIRE_SECS: i64 = 60;

#[cfg(not(target_arch = "wasm32"))]
fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("", "", "rerun")
}

pub mod api;

const ISSUER_URL_BASE: &str = "https://api.workos.com/user_management";
const JWKS_URL_BASE: &str = "https://api.workos.com/sso/jwks";

// TODO(jan): This is the client ID for the WorkOS public staging environment
// should be replaced by our actual client ID at some point.
// When doing so, don't forget to replace it everywhere. :)
const WORKOS_CLIENT_ID: &str = match option_env!("WORKOS_CLIENT_ID") {
    Some(v) => v,
    None => "client_01JZ3JVQW6JNVXME6HV9G4VR0H",
};
pub const DEFAULT_ISSUER: &str = const_format::concatcp!(ISSUER_URL_BASE, "/", WORKOS_CLIENT_ID);
const JWKS_URL: &str = const_format::concatcp!(JWKS_URL_BASE, "/", WORKOS_CLIENT_ID);

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

#[allow(dead_code)] // fields may become used at some point in the near future
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

#[allow(dead_code)] // fields may become used at some point in the near future
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

pub enum Status {
    Valid,
    NeedsRefresh,
}

#[derive(Debug, thiserror::Error)]
pub enum ContextLoadError {
    #[error("failed to read credentials: {0}")]
    FileRead(#[from] std::io::Error),

    #[error("failed to deserialize credentials: {0}")]
    Deserialize(#[from] serde_json::Error),

    #[error("could not find a valid config location, please ensure $HOME is set")]
    UnknownConfigLocation,

    #[error("failed to fetch JWKS: {0}")]
    FetchJwks(#[from] FetchJwksError),
}

pub struct AuthContext {
    pub jwks: Arc<JwkSet>,
}

impl AuthContext {
    pub async fn load() -> Result<Self, ContextLoadError> {
        // TODO(jan): for server usage, we should cache in some other way
        // TODO(jan): set our own TTL for this, refetch when it's about to expire

        // 1. try to load from disk cache
        match Self::load_from_cache() {
            Ok(cached) => {
                re_log::trace!("auth context loaded from cache");
                return Ok(cached);
            }
            Err(err) => {
                re_log::debug!("{err}");
            }
        }

        // 2. fetch and store in disk cache
        let this = Self::fetch().await?;
        re_log::trace!("auth context loaded from remote");
        this.store_in_cache();

        Ok(this)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn path() -> Option<std::path::PathBuf> {
        const FILENAME: &str = "context.json";

        let project_dirs = project_dirs()?;
        let path = project_dirs.cache_dir().join(FILENAME);
        Some(path)
    }

    async fn fetch() -> Result<Self, FetchJwksError> {
        let res = ehttp::fetch_async(ehttp::Request::get(JWKS_URL))
            .await
            .map_err(FetchJwksError::Request)?;

        if !res.ok {
            return Err(FetchJwksError::Request(
                res.text().unwrap_or(&res.status_text).to_owned(),
            ));
        }

        let jwks = serde_json::from_slice(&res.bytes)?;
        Ok(Self {
            jwks: Arc::new(jwks),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_cache() -> Result<Self, ContextLoadError> {
        let path = Self::path().ok_or(ContextLoadError::UnknownConfigLocation)?;
        let jwks = match std::fs::read_to_string(&path) {
            Ok(data) => data,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                re_log::debug!("`{}` not found", path.display());
                return Err(err.into());
            }
            Err(err) => return Err(err.into()),
        };
        let jwks = serde_json::from_str(&jwks)?;
        Ok(Self {
            jwks: Arc::new(jwks),
        })
    }

    // TODO(jan): implement when integrating directly in the Viewer
    #[cfg(target_arch = "wasm32")]
    fn load_from_cache() -> Result<Self, ContextLoadError> {
        unimplemented!()
    }

    /// Note: This is just a cache, so we're ok with it failing.
    #[cfg(not(target_arch = "wasm32"))]
    fn store_in_cache(&self) {
        use re_log::ResultExt as _;

        let path = match Self::path().ok_or(ContextLoadError::UnknownConfigLocation) {
            Ok(path) => path,
            Err(err) => {
                re_log::debug!("failed to get config directory: {err}");
                return;
            }
        };
        let Some(content) = serde_json::to_string_pretty(&*self.jwks).ok_or_log_error() else {
            return;
        };
        if let Err(err) = std::fs::write(&path, content) {
            re_log::warn!("Failed to write to cache file {path:?}: {err}");
        }
    }

    // TODO(jan): implement when integrating directly in the Viewer
    #[cfg(target_arch = "wasm32")]
    fn store_in_cache(&self) {
        unimplemented!()
    }

    /// Validate a JWT.
    ///
    /// Returns `Ok(None)` if the token is expired.
    pub fn verify_token(&self, jwt: Jwt) -> Result<Option<AccessToken>, VerifyError> {
        // 1. Decode header to get `kid`
        let header = decode_header(jwt.as_str())?;
        let kid = header.kid.as_ref().ok_or(VerifyError::MissingKeyId)?;
        let key = self
            .jwks
            .find(kid)
            .ok_or_else(|| VerifyError::KeyNotFound { id: kid.clone() })?;
        let key = DecodingKey::from_jwk(key)?;

        // 2. Verify token
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
}

/// In-memory credential storage
pub struct Credentials {
    user: User,
    refresh_token: RefreshToken,
    access_token: Option<AccessToken>,
}

impl Credentials {
    /// Load credentials from disk.
    ///
    /// Returns `None` if they don't exist.
    pub fn load() -> Result<Option<Self>, CredentialsError> {
        re_log::trace!("loading credentials");
        let config = CredentialsConfig::load()?;

        match config {
            Some(config) => Ok(Some(Self {
                user: config.user,
                refresh_token: config.refresh,
                access_token: config.access,
            })),
            None => Ok(None),
        }
    }

    /// Store credentials on the machine.
    pub fn store(&self) -> Result<(), CredentialsStoreError> {
        re_log::trace!("storing credentials");
        CredentialsConfig {
            refresh: self.refresh_token.private_clone(),
            access: self.access_token.clone(),
            user: self.user.clone(),
        }
        .store()
    }

    /// Refresh credentials if they are expired or invalid.
    pub async fn refresh_if_needed(
        &mut self,
        context: &AuthContext,
    ) -> Result<(), CredentialsError> {
        // If we don't have a token yet, generate one.
        let Some(token) = &self.access_token else {
            re_log::trace!("no access token yet");
            return self.refresh(context).await;
        };

        // If we have a token, ensure that it's valid, otherwise generate a new one.
        match context.verify_token(token.jwt()) {
            Ok(Some(_)) => {} // All good
            Ok(None) => {
                // Needs a refresh
                re_log::trace!("expired access token, refreshing");
                return self.refresh(context).await;
            }
            Err(err) => {
                // Invalid for some reason, let's try a refresh
                re_log::error!("{err}");
                return self.refresh(context).await;
            }
        }

        Ok(())
    }

    /// Refresh credentials.
    ///
    /// This revokes existing tokens, they should no longer be used for anything.
    ///
    /// This does not check if it's necessary, for that use [`Self::refresh_if_needed`].
    pub async fn refresh(&mut self, context: &AuthContext) -> Result<(), CredentialsError> {
        re_log::trace!("refreshing access token");
        let res = api::refresh(&self.refresh_token).await?;

        self.refresh_token = RefreshToken(res.refresh_token);
        self.access_token = Some(
            context
                .verify_token(Jwt(res.access_token))?
                .expect("freshly generated token should not be expired"),
        );
        self.user = res.user;

        Ok(())
    }

    #[allow(dead_code)] // only used on CLI path, causes warnings downstream
    /// Verifies that contents of `res` are valid and produces [`Credentials`].
    ///
    /// Assumes that tokens are freshly generated and are not about to expire.
    pub(crate) fn verify_auth_response(
        context: &AuthContext,
        res: api::AuthenticationResponse,
    ) -> Result<Self, CredentialsError> {
        Ok(Self {
            user: res.user,
            refresh_token: RefreshToken(res.refresh_token),
            access_token: Some(
                context
                    .verify_token(Jwt(res.access_token))?
                    .expect("freshly generated token should not be expired"),
            ),
        })
    }

    pub fn access_token(&self) -> Option<&AccessToken> {
        self.access_token.as_ref()
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
///
/// To produce one from a JWT, use [`AuthContext::verify_token`].
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
        // Time in seconds since unix epoch
        let now = jsonwebtoken::get_current_timestamp() as i64;
        let seconds_left = now - self.expires_at;
        seconds_left <= SOFT_EXPIRE_SECS
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub(crate) struct RefreshToken(String);

impl RefreshToken {
    fn private_clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Persisted on disk
#[derive(serde::Deserialize, serde::Serialize)]
struct CredentialsConfig {
    refresh: RefreshToken,
    access: Option<AccessToken>,
    user: User,
}

impl CredentialsConfig {
    #[cfg(not(target_arch = "wasm32"))]
    fn path() -> Option<std::path::PathBuf> {
        const FILENAME: &str = "credentials.json";

        let project_dirs = project_dirs()?;
        Some(project_dirs.config_local_dir().join(FILENAME))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load() -> Result<Option<Self>, CredentialsError> {
        let path = Self::path().ok_or(CredentialsError::UnknownConfigLocation)?;
        re_log::trace!("credentials load from `{}`", path.display());
        let data = match std::fs::read_to_string(&path) {
            Ok(data) => data,
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => return Ok(None),
                _ => return Err(err.into()),
            },
        };

        Ok(serde_json::from_str(&data)?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn store(&self) -> Result<(), CredentialsStoreError> {
        let path = Self::path().ok_or(CredentialsStoreError::UnknownConfigLocation)?;
        re_log::trace!("credentials store in `{}`", path.display());
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn load() -> Result<Option<Self>, CredentialsError> {
        todo!("persist in localstorage")
    }

    #[cfg(target_arch = "wasm32")]
    fn store(&self) -> Result<(), CredentialsStoreError> {
        todo!("persist in localstorage")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialsError {
    #[error("failed to read: {0}")]
    FileRead(#[from] std::io::Error),

    #[error("failed to deserialize: {0}")]
    Deserialize(#[from] serde_json::Error),

    #[error("could not find a valid config location, ensure $HOME is set")]
    UnknownConfigLocation,

    #[error("failed to verify: {0}")]
    Verify(#[from] VerifyError),

    #[error("failed to request new access token: {0}")]
    Http(#[from] api::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialsStoreError {
    #[error("failed to write file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to serialize credentials")]
    Serialize(#[from] serde_json::Error),

    #[error("could not find a valid config location, ensure $HOME is set")]
    UnknownConfigLocation,
}
