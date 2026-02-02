use crate::Jwt;

#[derive(Debug, thiserror::Error)]
pub enum CredentialsProviderError {
    #[error("session expired; please login using `rerun auth login`")]
    SessionExpired,

    #[error("{0}")]
    Custom(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl CredentialsProviderError {
    pub fn custom(inner: impl Into<Box<dyn std::error::Error + Send + Sync + 'static>>) -> Self {
        Self::Custom(inner.into())
    }
}

#[async_trait::async_trait]
pub trait CredentialsProvider: std::fmt::Debug {
    async fn get_token(&self) -> Result<Option<Jwt>, CredentialsProviderError>;
}

/// Provider which always returns the same token.
#[derive(Debug)]
pub struct StaticCredentialsProvider {
    token: Jwt,
}

impl StaticCredentialsProvider {
    pub fn new(token: Jwt) -> Self {
        Self { token }
    }
}

#[async_trait::async_trait]
impl CredentialsProvider for StaticCredentialsProvider {
    async fn get_token(&self) -> Result<Option<Jwt>, CredentialsProviderError> {
        Ok(Some(self.token.clone()))
    }
}

#[cfg(feature = "oauth")]
pub use oauth::{CliCredentialsProvider, subscribe_auth_changes};

#[cfg(feature = "oauth")]
pub(crate) mod oauth {
    use super::{CredentialsProvider, CredentialsProviderError, Jwt};
    use crate::oauth;
    use crate::oauth::{Credentials, load_and_refresh_credentials};
    use tokio::sync::RwLock;

    // We only want to keep a single instance of credentials in memory,
    // so we store them in a static.
    static CACHE: RwLock<Option<Credentials>> = RwLock::const_new(None);

    type AuthCallback = Box<dyn Fn(Option<oauth::User>) + Send>;
    static AUTH_SUBSCRIBERS: parking_lot::Mutex<Vec<AuthCallback>> =
        parking_lot::Mutex::new(Vec::new());

    pub(crate) fn auth_update(user: Option<&oauth::User>) {
        let subscribers = AUTH_SUBSCRIBERS.lock();
        for sub in &*subscribers {
            sub(user.cloned());
        }
    }

    /// Listen for changes to the authentication state.
    ///
    /// NOTE: You must call [`CliCredentialsProvider::get_token`] once after subscribing to get the
    /// initial state.
    pub fn subscribe_auth_changes(callback: impl Fn(Option<oauth::User>) + Send + 'static) {
        let mut subscribers = AUTH_SUBSCRIBERS.lock();
        subscribers.push(Box::new(callback));
    }

    /// Provider which uses `OAuth` credentials stored on the user's machine.
    #[derive(Debug, Default)]
    pub struct CliCredentialsProvider {
        _private: (),
    }

    impl CliCredentialsProvider {
        pub fn new() -> Self {
            Self::default()
        }
    }

    #[async_trait::async_trait]
    impl CredentialsProvider for CliCredentialsProvider {
        async fn get_token(&self) -> Result<Option<Jwt>, CredentialsProviderError> {
            {
                // Fast path: credentials are cached and not expired.
                if let Some(credentials) = &*CACHE.read().await
                    && !credentials.access_token().is_expired()
                {
                    return Ok(Some(credentials.access_token().jwt()));
                }
            }

            // Slow path: credentials are either not cached, or expired. Either way we must refresh:
            let mut cache = CACHE.write().await;

            // It's possible that a different thread already refreshed the token while
            // we were waiting for the write lock, so we have to immediately check again.
            if let Some(credentials) = &*cache
                && !credentials.access_token().is_expired()
            {
                // Early-out in this case.
                return Ok(Some(credentials.access_token().jwt()));
            }

            re_log::debug!("loading and refreshing credentials");

            // Now we have exclusive access, and credentials haven't been refreshed yet:
            match load_and_refresh_credentials().await {
                Ok(Some(credentials)) => {
                    // Success: cache credentials and return the token.
                    let token = credentials.access_token().jwt();
                    auth_update(Some(credentials.user()));
                    *cache = Some(credentials);
                    Ok(Some(token))
                }

                Ok(None) => {
                    re_log::debug!("no credentials available");

                    auth_update(None);

                    // There are no credentials stored on disk, so the user has not logged in yet.
                    // We represent that by saying there is no token:
                    Ok(None)
                }

                // TODO(jan): this needs to handle the case where the refresh token expired
                Err(err) => {
                    auth_update(None);
                    Err(CredentialsProviderError::Custom(err.into()))
                }
            }
        }
    }
}
