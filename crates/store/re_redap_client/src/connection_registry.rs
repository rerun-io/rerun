use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;

use itertools::Itertools as _;
use tokio::sync::RwLock;
use tonic::Code;

use re_auth::Jwt;
use re_protos::cloud::v1alpha1::{EntryFilter, FindEntriesRequest};

use crate::TonicStatusError;
use crate::connection_client::GenericConnectionClient;
use crate::grpc::{ConnectionError, RedapClient, RedapClientInner};

/// This is the type of `ConnectionClient` used throughout the viewer, where the
/// `ConnectionRegistry` is used.
pub type ConnectionClient = GenericConnectionClient<RedapClientInner>;

//TODO(#11016): refactor this to achieve lazy auth retry.
pub struct ConnectionRegistry {
    /// The saved authentication tokens.
    ///
    /// These are the tokens explicitly set by the user, e.g. via `--token` or the UI. They may
    /// be persisted.
    ///
    /// When no saved token is available for a given server, we fall back to the `REDAP_TOKEN`
    /// envvar if set. See [`ConnectionRegistryHandle::client`].
    saved_tokens: HashMap<re_uri::Origin, Jwt>,

    /// Fallback token.
    ///
    /// If set, the fallback token is used when no specific token is registered for a given origin.
    fallback_token: Option<Jwt>,

    /// The cached clients.
    ///
    /// Clients are much cheaper to clone than create (since the latter involves establishing an
    /// actual TCP connection), so we keep them around once created.
    clients: HashMap<re_uri::Origin, RedapClient>,
}

impl ConnectionRegistry {
    /// Create a new connection registry and return a handle to it.
    #[expect(clippy::new_ret_no_self)] // intentional, to reflect the fact that this is a handle
    pub fn new() -> ConnectionRegistryHandle {
        ConnectionRegistryHandle {
            inner: Arc::new(RwLock::new(Self {
                saved_tokens: HashMap::new(),
                fallback_token: None,
                clients: HashMap::new(),
            })),
        }
    }
}

/// Possible errors when creating a connection.
#[derive(Debug, thiserror::Error)]
pub enum ClientConnectionError {
    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error("Connection error\nDetails:{0}")]
    Tonic(#[from] tonic::transport::Error),

    #[error("server is expecting an unencrypted connection (try `rerun+http://` if you are sure)")]
    UnencryptedServer,

    #[error("the server requires an authentication token, but none was provided\nDetails:{0}")]
    UnauthenticatedMissingToken(TonicStatusError),

    #[error("the server rejected the provided authentication token\nDetails:{0}")]
    UnauthenticatedBadToken(TonicStatusError),

    #[error("failed to validate credentials\nDetails:{0}")]
    AuthCheckError(TonicStatusError),
}

impl From<ConnectionError> for ClientConnectionError {
    fn from(value: ConnectionError) -> Self {
        match value {
            #[cfg(not(target_arch = "wasm32"))]
            ConnectionError::Tonic(err) => Self::Tonic(err),

            ConnectionError::UnencryptedServer => Self::UnencryptedServer,
        }
    }
}

impl ClientConnectionError {
    pub fn is_token_error(&self) -> bool {
        matches!(
            self,
            Self::UnauthenticatedMissingToken(_) | Self::UnauthenticatedBadToken(_)
        )
    }
}

/// Registry of all tokens and connections to the redap servers.
///
/// This registry is cheap to clone.
#[derive(Clone)]
pub struct ConnectionRegistryHandle {
    inner: Arc<RwLock<ConnectionRegistry>>,
}

impl ConnectionRegistryHandle {
    pub fn set_token(&self, origin: &re_uri::Origin, token: Jwt) {
        wrap_blocking_lock(|| {
            let mut inner = self.inner.blocking_write();
            inner.saved_tokens.insert(origin.clone(), token);
            inner.clients.remove(origin);
        });
    }

    pub fn token(&self, origin: &re_uri::Origin) -> Option<Jwt> {
        wrap_blocking_lock(|| {
            let inner = self.inner.blocking_read();
            inner.saved_tokens.get(origin).cloned()
        })
    }

    pub fn remove_token(&self, origin: &re_uri::Origin) {
        wrap_blocking_lock(|| {
            let mut inner = self.inner.blocking_write();
            inner.saved_tokens.remove(origin);
            inner.clients.remove(origin);
        });
    }

    pub fn set_fallback_token(&self, token: Jwt) {
        wrap_blocking_lock(|| {
            let mut inner = self.inner.blocking_write();
            inner.fallback_token = Some(token);
        });
    }

    /// Get a client for the given origin, creating one if it doesn't exist yet.
    ///
    /// Note: although `RedapClient` is cheap to clone, callsites should generally *not* hold on to
    /// client instances for longer than the immediate needs. In the future, authentication may
    /// require periodic tokens refresh, so it is necessary to always get a "fresh" client.
    ///
    /// If a token has already been registered for this origin, it will be used. Otherwise, it will attempt to
    /// use the following token, in this order:
    /// - The fallback token, if set via [`Self::set_fallback_token`].
    /// - The `REDAP_TOKEN` environment variable is set.
    ///
    /// Failing that, no token will be used.
    pub async fn client(
        &self,
        origin: re_uri::Origin,
    ) -> Result<ConnectionClient, ClientConnectionError> {
        // happy path
        {
            let inner = self.inner.read().await;
            if let Some(client) = inner.clients.get(&origin) {
                return Ok(ConnectionClient::new(client.clone()));
            }
        }

        // Don't hold the lock while creating the client - this may take a while and we may
        // want to read the tokens in the meantime for other purposes.
        let (saved_token, fallback_token) = {
            let inner = self.inner.read().await;
            (
                inner.saved_tokens.get(&origin).cloned(),
                inner.fallback_token.clone(),
            )
        };

        let token_to_try = [saved_token.clone(), fallback_token, get_token_from_env()]
            .into_iter()
            .flatten()
            .unique();

        let (raw_client, successful_token) =
            match Self::try_create_raw_client(origin.clone(), token_to_try).await {
                Ok(res) => res,
                Err(err) => {
                    // if we had a saved token, it doesn't work, so we forget about it
                    if err.is_token_error() {
                        let mut inner = self.inner.write().await;

                        // make sure that we're not deleting some token that another thread might
                        // have just set
                        if inner.saved_tokens.get(&origin) == saved_token.as_ref() {
                            inner.saved_tokens.remove(&origin);
                        }
                    }

                    return Err(err);
                }
            };
        let client = ConnectionClient::new(raw_client.clone());

        // We have a successful client, so we cache it and remember about the successful token.
        //
        // Note: because we only acquire the lock now, a race is possible where two threads
        // concurrently attempt to create the client and would override each-other's results. This
        // is acceptable since both should reach the same conclusion, and preferable that holding
        // the lock for the entire time, as the connection process can take a while.
        {
            let mut inner = self.inner.write().await;
            inner.clients.insert(origin.clone(), raw_client.clone());

            if successful_token != saved_token {
                if let Some(successful_token) = successful_token {
                    inner.saved_tokens.insert(origin.clone(), successful_token);
                } else {
                    inner.saved_tokens.remove(&origin);
                }
            }
        }

        Ok(client)
    }

    /// Try creating (and validating) a raw client using whatever token we might have available.
    ///
    /// If successful, returns both the client and the working token.
    async fn try_create_raw_client(
        origin: re_uri::Origin,
        possible_tokens: impl Iterator<Item = Jwt>,
    ) -> Result<(RedapClient, Option<Jwt>), ClientConnectionError> {
        let mut first_failed_token_attempt = None;

        for token in possible_tokens {
            let result = Self::create_and_validate_raw_client_with_token(
                origin.clone(),
                Some(token.clone()),
            )
            .await;

            match result {
                Ok(raw_client) => return Ok((raw_client, Some(token))),

                Err(err) if err.is_token_error() => {
                    // remember about the first occurrence of this error but continue trying other
                    // tokens
                    if first_failed_token_attempt.is_none() {
                        first_failed_token_attempt = Some(err);
                    }
                }

                Err(err) => return Err(err),
            }
        }

        // Everything failed, last ditch effort without a token.
        let result = Self::create_and_validate_raw_client_with_token(origin.clone(), None).await;

        match result {
            Ok(raw_client) => Ok((raw_client, None)),

            Err(err) => {
                // If we actually tried tokens, this error is more relevant.
                if let Some(first_failed_token_attempt) = first_failed_token_attempt {
                    Err(first_failed_token_attempt)
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn create_and_validate_raw_client_with_token(
        origin: re_uri::Origin,
        token: Option<Jwt>,
    ) -> Result<RedapClient, ClientConnectionError> {
        let mut raw_client = match crate::grpc::client(origin.clone(), token.clone()).await {
            Ok(raw_client) => raw_client,

            Err(err) => {
                return Err(err.into());
            }
        };

        // Call the version endpoint to check that authentication is successful. It's ok to do this
        // since we're caching the client, so we're not spamming such a request unnecessarily.
        // TODO(rerun-io/dataplatform#1069): use the `whoami` endpoint instead when it exists.
        let request_result = raw_client
            .find_entries(FindEntriesRequest {
                filter: Some(EntryFilter {
                    id: None,
                    name: None,
                    entry_kind: None,
                }),
            })
            .await;

        match request_result {
            // catch unauthenticated errors and forget the token if they happen
            Err(err) if err.code() == Code::Unauthenticated => {
                if token.is_none() {
                    Err(ClientConnectionError::UnauthenticatedMissingToken(
                        err.into(),
                    ))
                } else {
                    Err(ClientConnectionError::UnauthenticatedBadToken(err.into()))
                }
            }

            Err(err) => Err(ClientConnectionError::AuthCheckError(err.into())),

            Ok(_) => Ok(raw_client),
        }
    }

    /// Dump all tokens for persistence purposes.
    pub fn dump_tokens(&self) -> SerializedTokens {
        wrap_blocking_lock(|| {
            let this = self.inner.blocking_read();
            let per_origin = this
                .saved_tokens
                .iter()
                .map(|(origin, token)| (origin.clone(), token.to_string()))
                .collect();
            let fallback = this.fallback_token.clone().map(|v| v.to_string());

            SerializedTokens {
                tokens_per_origin: per_origin,
                fallback_token: fallback,
            }
        })
    }

    /// Load tokens from persistence.
    ///
    /// IMPORTANT: This will NOT overwrite any existing tokens, since it is assumed that existing
    /// tokens were explicitly set by the user (e.g. with `--token`).
    pub fn load_tokens(&self, tokens: SerializedTokens) {
        wrap_blocking_lock(|| {
            let mut inner = self.inner.blocking_write();
            for (origin, token) in tokens.tokens_per_origin {
                if let Entry::Vacant(e) = inner.saved_tokens.entry(origin.clone()) {
                    if let Ok(jwt) = Jwt::try_from(token) {
                        e.insert(jwt);
                    } else {
                        re_log::debug!("Failed to parse token for origin {origin}");
                    }
                } else {
                    re_log::trace!("Ignoring token for origin {origin} as it is already set");
                }
            }

            if let Some(fallback_token) = tokens
                .fallback_token
                .and_then(|token| Jwt::try_from(token).ok())
                && inner.fallback_token.is_none()
            {
                inner.fallback_token = Some(fallback_token);
            }
        });
    }
}

/// Wraps code using blocking tokio locks.
///
/// This is required if the calling code is running on an async executor thread, see e.g.
/// [`tokio::sync::RwLock::blocking_write`].
#[inline]
fn wrap_blocking_lock<F, R>(inner: F) -> R
where
    F: FnOnce() -> R,
{
    #[cfg(not(target_arch = "wasm32"))]
    let res = tokio::task::block_in_place(inner);

    #[cfg(target_arch = "wasm32")]
    let res = inner();

    res
}

// ---

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializedTokens {
    tokens_per_origin: Vec<(re_uri::Origin, String)>,
    fallback_token: Option<String>,
}

// ---

#[cfg(not(target_arch = "wasm32"))]
fn get_token_from_env() -> Option<Jwt> {
    std::env::var("REDAP_TOKEN")
        .map_err(|err| match err {
            std::env::VarError::NotPresent => {}
            std::env::VarError::NotUnicode(..) => {
                re_log::warn_once!("REDAP_TOKEN env var is malformed: {err}");
            }
        })
        .and_then(|t| {
            re_auth::Jwt::try_from(t).map_err(|err| {
                re_log::warn_once!(
                    "REDAP_TOKEN env var is present, but the token is invalid: {err}"
                );
            })
        })
        .ok()
}

#[cfg(target_arch = "wasm32")]
fn get_token_from_env() -> Option<Jwt> {
    None
}
