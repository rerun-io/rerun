use serde::{Deserialize, Serialize};

use crate::token::JwtDecodeError;
use crate::{Jwt, Permission};

pub mod api;
mod storage;

#[cfg(not(target_arch = "wasm32"))]
pub mod login_flow;

/// Tokens with fewer than this number of seconds left before expiration
/// are considered expired. This ensures tokens don't become expired
/// during network transit.
const SOFT_EXPIRE_SECS: i64 = 60;

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
#[error("failed to load credentials: {0}")]
pub struct CredentialsClearError(#[from] storage::ClearError);

/// Result of a successful [`clear_credentials`] call that had stored credentials.
pub struct LogoutOutcome {
    /// The `WorkOS` logout URL to open in the user's browser.
    pub logout_url: String,

    /// On native, a handle to the background callback server thread.
    ///
    /// Join this handle to keep the process alive until the browser has
    /// loaded the "logged out" landing page (or the server times out).
    #[cfg(not(target_arch = "wasm32"))]
    pub server_handle: Option<std::thread::JoinHandle<()>>,
}

/// Clear stored credentials and return the `WorkOS` logout URL, if available.
///
/// On native, this also starts a local callback server so the browser has
/// somewhere to redirect after the `WorkOS` session is cleared.
///
/// The logout URL should be opened in the user's browser to also end the
/// `WorkOS` session. If no credentials were stored (or the session ID could
/// not be determined), `Ok(None)` is returned.
pub fn clear_credentials() -> Result<Option<LogoutOutcome>, CredentialsClearError> {
    // Load credentials before clearing so we can extract the session ID.
    let outcome = storage::load().ok().flatten().map(|creds| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            // On native, start a local callback server so WorkOS can redirect
            // back to a "logged out" landing page.
            match crate::callback_server::start_logout_server(&creds.claims.sid) {
                Ok((url, handle)) => LogoutOutcome {
                    logout_url: url,
                    server_handle: Some(handle),
                },
                Err(err) => {
                    re_log::warn!("Failed to start logout callback server: {err}");
                    LogoutOutcome {
                        logout_url: api::logout_url(&creds.claims.sid, None),
                        server_handle: None,
                    }
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // On web, redirect to /signed-out on the current origin after logout.
            let return_to = web_sys::window()
                .and_then(|w| w.location().origin().ok())
                .map(|origin| format!("{origin}/signed-out"));
            LogoutOutcome {
                logout_url: api::logout_url(&creds.claims.sid, return_to.as_deref()),
            }
        }
    });

    storage::clear()?;

    crate::credentials::oauth::auth_update(None);

    Ok(outcome)
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialsRefreshError {
    #[error("failed to refresh credentials: {0}")]
    Api(#[from] api::Error),

    #[error("failed to store credentials: {0}")]
    Store(#[from] storage::StoreError),

    #[error("failed to deserialize credentials: {0}")]
    MalformedToken(#[from] JwtDecodeError),

    #[error("no refresh token available")]
    NoRefreshToken,
}

/// Refresh credentials if they are expired.
pub async fn refresh_credentials(
    credentials: Credentials,
) -> Result<Credentials, CredentialsRefreshError> {
    refresh_credentials_with_org(credentials, None).await
}

/// Refresh credentials, optionally switching to a different organization.
///
/// If `organization_id` is `Some`, a refresh is always performed (even if the
/// token hasn't expired) to obtain a token scoped to the specified org.
pub async fn refresh_credentials_with_org(
    credentials: Credentials,
    organization_id: Option<&str>,
) -> Result<Credentials, CredentialsRefreshError> {
    // If no org switch is requested, don't refresh unless the access token has expired
    if organization_id.is_none() && !credentials.access_token().is_expired() {
        re_log::debug!(
            "skipping credentials refresh: credentials expire in {} seconds",
            credentials.access_token().remaining_duration_secs()
        );
        return Ok(credentials);
    }

    if organization_id.is_none() {
        re_log::debug!(
            "expired {} seconds ago",
            -credentials.access_token().remaining_duration_secs()
        );
    }

    let Some(refresh_token) = &credentials.refresh_token else {
        return Err(CredentialsRefreshError::NoRefreshToken);
    };

    let response = api::refresh(refresh_token, organization_id).await?;
    let credentials = Credentials::from_auth_response(response)?
        .ensure_stored()
        .map_err(|err| CredentialsRefreshError::Store(err.0))?;
    re_log::debug!("credentials refreshed successfully");
    Ok(credentials)
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialsError {
    #[error("failed to load credentials: {0}")]
    Load(#[from] CredentialsLoadError),

    #[error("{0}")]
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

    pub fn try_from_unverified_jwt(jwt: &Jwt) -> Result<Self, JwtDecodeError> {
        jwt.decode_claims()
    }
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

    // Refresh token is optional because it may not be available in some cases,
    // like the Jupyter notebook Wasm viewer. In that case, the SDK handles
    // token refreshes.
    refresh_token: Option<RefreshToken>,

    access_token: AccessToken,
    claims: RerunCloudClaims,
}

pub struct InMemoryCredentials(Credentials);

#[derive(Debug, thiserror::Error)]
#[error("failed to store credentials: {0}")]
pub struct CredentialsStoreError(#[from] storage::StoreError);

impl InMemoryCredentials {
    /// Ensure credentials are persisted to disk before using them.
    pub fn ensure_stored(self) -> Result<Credentials, CredentialsStoreError> {
        storage::store(&self.0)?;

        // Normally if re_analytics discovers this is a brand-new configuration,
        // we show an analytics diclaimer. But, during SDK usage with the Catalog
        // it's possible to hit this code-path during a first run in a new
        // environment. Given the user already has a Rerun identity (or else there
        // would be no credentials to store!), we assume they are already aware of
        // rerun analytics and do not need a disclaimer. They can still use the shell
        // to run `rerun analytics disable` if they wish to opt out.
        //
        // By manually forcing the creation of the analytics config we bypass the first_run check.
        if let Ok(config) = re_analytics::Config::load_or_default()
            && config.is_first_run()
        {
            config.save().ok();
        }

        // Link the analytics ID to the authenticated user
        re_analytics::record(|| re_analytics::event::SetPersonProperty {
            email: self.0.user.email.clone(),
            organization_id: self.0.claims.org_id.clone(),
        });

        crate::credentials::oauth::auth_update(Some(&self.0.user));

        Ok(self.0)
    }
}

impl Credentials {
    /// Deserializes credentials from an authentication response.
    ///
    /// Assumes the credentials are valid and not expired.
    ///
    /// The authentication response must come from a trusted source, such
    /// as the authentication API.
    pub fn from_auth_response(
        res: api::RefreshResponse,
    ) -> Result<InMemoryCredentials, JwtDecodeError> {
        let jwt = Jwt(res.access_token);
        let claims = RerunCloudClaims::try_from_unverified_jwt(&jwt)?;
        let access_token = AccessToken::try_from_unverified_jwt(jwt)?;
        Ok(InMemoryCredentials(Self {
            user: res.user,
            refresh_token: Some(RefreshToken(res.refresh_token)),
            access_token,
            claims,
        }))
    }

    /// Creates credentials from raw token strings.
    ///
    /// Warning: it does not check the signature of the access token.
    pub fn try_new(
        access_token: String,
        refresh_token: Option<String>,
        email: String,
    ) -> Result<InMemoryCredentials, JwtDecodeError> {
        let claims = RerunCloudClaims::try_from_unverified_jwt(&Jwt(access_token.clone()))?;

        let user = User {
            id: claims.sub.clone(),
            email,
        };
        let access_token = AccessToken {
            token: access_token,
            expires_at: claims.exp,
        };
        let refresh_token = refresh_token.map(RefreshToken);

        Ok(InMemoryCredentials(Self {
            user,
            refresh_token,
            access_token,
            claims,
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
    /// The token should come from a trusted source, like the Rerun auth API.
    pub(crate) fn try_from_unverified_jwt(jwt: Jwt) -> Result<Self, JwtDecodeError> {
        let claims = RerunCloudClaims::try_from_unverified_jwt(&jwt)?;
        Ok(Self {
            token: jwt.0,
            expires_at: claims.exp,
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

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub(crate) struct RefreshToken(String);

impl std::fmt::Debug for RefreshToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RefreshToken").field(&"…").finish()
    }
}
