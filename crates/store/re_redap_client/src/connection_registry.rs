use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;

use itertools::Itertools as _;
use tokio::sync::RwLock;
use tonic::Code;

use re_auth::Jwt;
use re_protos::cloud::v1alpha1::{EntryFilter, FindEntriesRequest};

use crate::connection_client::GenericConnectionClient;
use crate::grpc::{RedapClient, RedapClientInner};
use crate::{ApiError, TonicStatusError};

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
    saved_credentials: HashMap<re_uri::Origin, Credentials>,

    /// Fallback token.
    ///
    /// If set, the fallback token is used when no specific token is registered for a given origin.
    fallback_token: Option<Jwt>,

    /// Use stored Rerun Cloud credentials.
    ///
    /// If set, we always attempt to use these when no explicit token is provided,
    /// unless the origin is localhost.
    use_stored_credentials: bool,

    /// The cached clients.
    ///
    /// Clients are much cheaper to clone than create (since the latter involves establishing an
    /// actual TCP connection), so we keep them around once created.
    clients: HashMap<re_uri::Origin, RedapClient>,
}

impl ConnectionRegistry {
    /// Create a new connection registry and return a handle to it.
    ///
    /// This version explicitly disables using credentials stored on the local machine,
    /// which makes sense to do in test contexts, or where you don't want the connection
    /// registry "impersonating" the local user.
    ///
    /// Using stored credentials by default should be preferred in all other contexts,
    /// see [`Self::new_with_stored_credentials`].
    pub fn new_without_stored_credentials() -> ConnectionRegistryHandle {
        ConnectionRegistryHandle {
            inner: Arc::new(RwLock::new(Self {
                saved_credentials: HashMap::new(),
                fallback_token: None,
                use_stored_credentials: false,
                clients: HashMap::new(),
            })),
        }
    }

    /// Create a new connection registry and return a handle to it.
    ///
    /// This version explicitly enables using credentials stored on the local machine.
    pub fn new_with_stored_credentials() -> ConnectionRegistryHandle {
        ConnectionRegistryHandle {
            inner: Arc::new(RwLock::new(Self {
                saved_credentials: HashMap::new(),
                fallback_token: None,
                use_stored_credentials: true,
                clients: HashMap::new(),
            })),
        }
    }
}

/// Possible errors when creating a connection.
#[derive(Debug, thiserror::Error)]
pub enum ClientCredentialsError {
    #[error("the server requires an authentication token but none was provided\nDetails:{0}")]
    UnauthenticatedMissingToken(TonicStatusError),

    #[error("the server rejected the provided authentication token\nDetails:{0}")]
    UnauthenticatedBadToken(TonicStatusError),
}

impl ClientCredentialsError {
    #[inline]
    pub fn is_missing_token(&self) -> bool {
        matches!(self, Self::UnauthenticatedMissingToken(_))
    }

    #[inline]
    pub fn is_wrong_token(&self) -> bool {
        matches!(self, Self::UnauthenticatedBadToken(_))
    }
}

/// Registry of all tokens and connections to the redap servers.
///
/// This registry is cheap to clone.
#[derive(Clone)]
pub struct ConnectionRegistryHandle {
    inner: Arc<RwLock<ConnectionRegistry>>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Credentials {
    /// Explicit token
    Token(Jwt),

    /// Credentials from local storage
    Stored,
}

impl ConnectionRegistryHandle {
    pub fn set_token(&self, origin: &re_uri::Origin, token: Jwt) {
        wrap_blocking_lock(|| {
            let mut inner = self.inner.blocking_write();
            inner
                .saved_credentials
                .insert(origin.clone(), Credentials::Token(token));
            inner.clients.remove(origin);
        });
    }

    pub fn token(&self, origin: &re_uri::Origin) -> Option<Jwt> {
        wrap_blocking_lock(|| {
            let inner = self.inner.blocking_read();
            match inner.saved_credentials.get(origin).cloned() {
                Some(Credentials::Token(token)) => Some(token),
                _ => None,
            }
        })
    }

    pub fn remove_token(&self, origin: &re_uri::Origin) {
        wrap_blocking_lock(|| {
            let mut inner = self.inner.blocking_write();
            inner.saved_credentials.remove(origin);
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
    /// - Local credentials for Rerun Cloud
    ///
    /// Failing that, no token will be used.
    pub async fn client(&self, origin: re_uri::Origin) -> Result<ConnectionClient, ApiError> {
        // happy path
        {
            let inner = self.inner.read().await;
            if let Some(client) = inner.clients.get(&origin) {
                return Ok(ConnectionClient::new(client.clone()));
            }
        }

        // Don't hold the lock while creating the client - this may take a while and we may
        // want to read the tokens in the meantime for other purposes.
        let (saved_token, fallback_token, may_use_stored_credentials) = {
            let inner = self.inner.read().await;
            (
                inner.saved_credentials.get(&origin).cloned(),
                inner.fallback_token.clone(),
                inner.use_stored_credentials,
            )
        };

        let token_to_try = [
            saved_token.clone(),
            fallback_token.map(Credentials::Token),
            get_token_from_env().map(Credentials::Token),
        ]
        .into_iter()
        .flatten()
        .unique();

        // It's not common that you'd need credentials on localhost.
        let use_stored_credentials_on_localhost = std::env::var("USE_STORED_CREDENTIALS").is_ok();
        let actually_use_stored_credentials = !use_stored_credentials_on_localhost
            && !origin.is_localhost()
            && may_use_stored_credentials;

        if actually_use_stored_credentials != may_use_stored_credentials {
            re_log::debug!(
                "not using stored credentials because origin is localhost; set USE_STORED_CREDENTIALS=1 if this is desired"
            );
        }

        let client_result = if actually_use_stored_credentials {
            Self::try_create_raw_client(
                origin.clone(),
                token_to_try.chain(std::iter::once(Credentials::Stored)),
            )
            .await
        } else {
            Self::try_create_raw_client(origin.clone(), token_to_try).await
        };

        let (raw_client, successful_token) = match client_result {
            Ok(res) => res,
            Err(err) => {
                // if we had a saved token, it doesn't work, so we forget about it
                if err.is_client_credentials_error() {
                    let mut inner = self.inner.write().await;

                    // make sure that we're not deleting some token that another thread might
                    // have just set
                    if inner.saved_credentials.get(&origin) == saved_token.as_ref() {
                        inner.saved_credentials.remove(&origin);
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
                    inner
                        .saved_credentials
                        .insert(origin.clone(), successful_token);
                } else {
                    inner.saved_credentials.remove(&origin);
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
        possible_credentials: impl Iterator<Item = Credentials>,
    ) -> Result<(RedapClient, Option<Credentials>), ApiError> {
        let mut first_failed_attempt = None;

        for credentials in possible_credentials {
            let result = Self::create_and_validate_raw_client_with_token(
                origin.clone(),
                Some(credentials.clone()),
            )
            .await;

            match result {
                Ok(raw_client) => return Ok((raw_client, Some(credentials))),

                Err(err) if err.is_client_credentials_error() => {
                    // remember about the first occurrence of this error but continue trying other
                    // tokens
                    if first_failed_attempt.is_none() {
                        first_failed_attempt = Some(err);
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
                if let Some(first_failed_attempt) = first_failed_attempt {
                    Err(first_failed_attempt)
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn create_and_validate_raw_client_with_token(
        origin: re_uri::Origin,
        credentials: Option<Credentials>,
    ) -> Result<RedapClient, ApiError> {
        let provider: Option<Arc<dyn re_auth::credentials::CredentialsProvider + Send + Sync>> =
            match &credentials {
                Some(Credentials::Token(token)) => Some(Arc::new(
                    re_auth::credentials::StaticCredentialsProvider::new(token.clone()),
                )),
                Some(Credentials::Stored) => {
                    Some(Arc::new(re_auth::credentials::CliCredentialsProvider::new()))
                }
                None => None,
            };

        let mut raw_client = crate::grpc::client(origin.clone(), provider).await?;

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
                if credentials.is_none() {
                    Err(ApiError::credentials(
                        ClientCredentialsError::UnauthenticatedMissingToken(err.into()),
                        "verifying credentials",
                    ))
                } else {
                    Err(ApiError::credentials(
                        ClientCredentialsError::UnauthenticatedBadToken(err.into()),
                        "verifying credentials",
                    ))
                }
            }

            Err(err) => Err(ApiError::tonic(err, "verifying credentials")),

            Ok(_) => Ok(raw_client),
        }
    }

    /// Dump all tokens for persistence purposes.
    pub fn dump_tokens(&self) -> SerializedTokens {
        wrap_blocking_lock(|| {
            let this = self.inner.blocking_read();
            let per_origin = this
                .saved_credentials
                .iter()
                .map(|(origin, credentials)| {
                    (
                        origin.clone(),
                        match credentials {
                            Credentials::Token(jwt) => SerializedCredentials::Jwt(jwt.to_string()),
                            Credentials::Stored => SerializedCredentials::Stored,
                        },
                    )
                })
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
                if let Entry::Vacant(e) = inner.saved_credentials.entry(origin.clone()) {
                    match token {
                        SerializedCredentials::Stored => {
                            e.insert(Credentials::Stored);
                        }
                        SerializedCredentials::Jwt(token) => {
                            if let Ok(jwt) = Jwt::try_from(token) {
                                e.insert(Credentials::Token(jwt));
                            } else {
                                re_log::debug!("Failed to parse token for origin {origin}");
                            }
                        }
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
    tokens_per_origin: Vec<(re_uri::Origin, SerializedCredentials)>,
    fallback_token: Option<String>,
}

#[derive(Debug, Clone)]
enum SerializedCredentials {
    Stored,
    Jwt(String),
}

impl<'de> serde::Deserialize<'de> for SerializedCredentials {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        if s == "stored" {
            Ok(Self::Stored)
        } else {
            Ok(Self::Jwt(s))
        }
    }
}

impl serde::Serialize for SerializedCredentials {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Stored => serializer.serialize_str("stored"),
            Self::Jwt(token) => serializer.serialize_str(token),
        }
    }
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
