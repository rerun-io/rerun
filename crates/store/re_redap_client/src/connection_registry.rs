use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::error::Error as _;
use std::sync::Arc;

use re_auth::Jwt;
use re_auth::credentials::CredentialsProviderError;
use re_protos::cloud::v1alpha1::{EntryFilter, FindEntriesRequest};
use re_uri::Origin;
use tokio::sync::RwLock;
use tonic::Code;

use crate::connection_client::GenericConnectionClient;
use crate::grpc::{RedapClient, RedapClientInner};
use crate::{ApiError, ApiResult, TonicStatusError};

/// Returns a suggested host if the user likely forgot the "api." prefix.
///
/// This detects the common mistake of connecting to `xxx.cloud.rerun.io` instead of
/// `api.xxx.cloud.rerun.io`.
fn suggest_api_prefix(origin: &Origin) -> Option<String> {
    let host = origin.format_host();
    if !host.starts_with("api.") && host.ends_with(".cloud.rerun.io") {
        Some(format!("api.{host}"))
    } else {
        None
    }
}

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

    /// The cached clients.
    ///
    /// Clients are much cheaper to clone than create (since the latter involves establishing an
    /// actual TCP connection), so we keep them around once created.
    clients: HashMap<re_uri::Origin, RedapClient>,
}

impl ConnectionRegistry {
    /// Create a new connection registry and return a handle to it.
    ///
    /// This version uses stored credentials by default if they are available.
    /// You should prefer to use this instead of [`Self::new_without_stored_credentials`]
    /// if there is no reason not to.
    pub fn new_with_stored_credentials() -> ConnectionRegistryHandle {
        ConnectionRegistryHandle {
            inner: Arc::new(RwLock::new(Self {
                saved_credentials: HashMap::new(),
                fallback_token: None,
                clients: HashMap::new(),
            })),
            use_stored_credentials: true,
        }
    }

    /// Create a new connection registry and return a handle to it.
    ///
    /// This version does not use stored credentials by default if they are available.
    /// You should prefer to use [`Self::new_with_stored_credentials`] instead,
    /// if there is no reason not to.
    pub fn new_without_stored_credentials() -> ConnectionRegistryHandle {
        ConnectionRegistryHandle {
            inner: Arc::new(RwLock::new(Self {
                saved_credentials: HashMap::new(),
                fallback_token: None,
                clients: HashMap::new(),
            })),
            use_stored_credentials: false,
        }
    }
}

/// Possible errors when creating a connection.
#[derive(Debug, thiserror::Error)]
pub enum ClientCredentialsError {
    #[error("error when refreshing credentials\nDetails: {0}")]
    RefreshError(TonicStatusError),

    #[error("the credentials are expired")]
    SessionExpired,

    #[error("the server requires an authentication token but none was provided\nDetails: {0}")]
    UnauthenticatedMissingToken(TonicStatusError),

    #[error("the server rejected the provided authentication token\nDetails: {status}")]
    UnauthenticatedBadToken {
        status: TonicStatusError,
        credentials: SourcedCredentials,
    },

    #[error("{0}")]
    HostMismatch(re_auth::HostMismatchError),
}

/// Registry of all tokens and connections to the redap servers.
///
/// This registry is cheap to clone.
#[derive(Clone)]
pub struct ConnectionRegistryHandle {
    inner: Arc<RwLock<ConnectionRegistry>>,

    /// Whether to use credentials stored on the host machine by default.
    /// Since some tests run on a single-threaded tokio runtime and this is never updated,
    /// it lives outside the `RwLock`.
    use_stored_credentials: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Credentials {
    /// Explicit token
    Token(Jwt),

    /// Credentials from local storage
    Stored,
}

#[derive(Clone, Debug)]
pub enum CredentialSource {
    PerOrigin,
    Fallback,
    EnvVar,
}

impl std::fmt::Display for CredentialSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PerOrigin => f.write_str("per-origin"),
            Self::Fallback => f.write_str("fallback"),
            Self::EnvVar => f.write_str("env-var"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SourcedCredentials {
    pub source: CredentialSource,
    pub credentials: Credentials,
}

impl ConnectionRegistryHandle {
    pub fn set_credentials(&self, origin: &re_uri::Origin, credentials: Credentials) {
        wrap_blocking_lock(|| {
            let mut inner = self.inner.blocking_write();
            inner.saved_credentials.insert(origin.clone(), credentials);
            inner.clients.remove(origin);
        });
    }

    pub fn credentials(&self, origin: &re_uri::Origin) -> Option<Credentials> {
        wrap_blocking_lock(|| {
            let inner = self.inner.blocking_read();
            inner.saved_credentials.get(origin).cloned()
        })
    }

    pub fn remove_credentials(&self, origin: &re_uri::Origin) {
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

    pub fn should_use_stored_credentials(&self) -> bool {
        self.use_stored_credentials
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
    pub async fn client(&self, origin: re_uri::Origin) -> ApiResult<ConnectionClient> {
        // happy path
        {
            let inner = self.inner.read().await;
            if let Some(client) = inner.clients.get(&origin) {
                return Ok(ConnectionClient::new(client.clone()));
            }
        }

        // Don't hold the lock while creating the client - this may take a while and we may
        // want to read the tokens in the meantime for other purposes.
        let (saved_credentials, fallback_token) = {
            let inner = self.inner.read().await;
            (
                inner.saved_credentials.get(&origin).cloned(),
                inner.fallback_token.clone(),
            )
        };

        let mut credentials_to_try = vec![];

        let add_cred = |creds: &mut Vec<SourcedCredentials>, cred: SourcedCredentials| {
            if !creds.iter().any(|c| c.credentials == cred.credentials) {
                creds.push(cred);
            }
        };

        for cred in [
            saved_credentials
                .clone()
                .map(|credentials| SourcedCredentials {
                    source: CredentialSource::PerOrigin,
                    credentials,
                }),
            fallback_token.clone().map(|token| SourcedCredentials {
                source: CredentialSource::Fallback,
                credentials: Credentials::Token(token),
            }),
            get_token_from_env().map(|token| SourcedCredentials {
                source: CredentialSource::EnvVar,
                credentials: Credentials::Token(token),
            }),
        ]
        .into_iter()
        .flatten()
        {
            add_cred(&mut credentials_to_try, cred);
        }

        let (raw_client, successful_credentials) =
            Self::try_create_raw_client(origin.clone(), credentials_to_try.into_iter()).await?;

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

            let successful_credentials = successful_credentials.map(|c| c.credentials);

            if successful_credentials != saved_credentials {
                if let Some(successful_token) = successful_credentials {
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
        possible_credentials: impl Iterator<Item = SourcedCredentials>,
    ) -> ApiResult<(RedapClient, Option<SourcedCredentials>)> {
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
        credentials: Option<SourcedCredentials>,
    ) -> ApiResult<RedapClient> {
        // Check the token's allowed hosts before wrapping it in a provider that
        // would blindly attach it to every outgoing request. If the host
        // doesn't match, return a credentials error so the caller can skip
        // this credential and try the next one.
        let host = origin.host.to_string();
        let provider: Option<Arc<dyn re_auth::credentials::CredentialsProvider + Send + Sync>> =
            match credentials.as_ref().map(|c| &c.credentials) {
                Some(Credentials::Token(token)) => {
                    token.for_host(&host).map_err(|err| {
                        ApiError::credentials_with_source(
                            ClientCredentialsError::HostMismatch(err),
                            format!("token not allowed for host '{host}'"),
                        )
                    })?;
                    Some(Arc::new(
                        re_auth::credentials::StaticCredentialsProvider::new(token.clone()),
                    ))
                }
                Some(Credentials::Stored) => {
                    // For stored credentials, load the token to check its allowed hosts
                    // before committing to using it.
                    if let Ok(Some(c)) = re_auth::oauth::load_credentials() {
                        c.access_token().jwt().for_host(&host).map_err(|err| {
                            ApiError::credentials_with_source(
                                ClientCredentialsError::HostMismatch(err),
                                format!("stored token not allowed for host '{host}'"),
                            )
                        })?;
                    }
                    Some(Arc::new(re_auth::credentials::CliCredentialsProvider::new()))
                }
                None => None,
            };

        // It's a common mistake to connect to `asdf.rerun.io` instead of `api.asdf.rerun.io`,
        // so if what we're trying to connect to is not a valid Rerun server, then cut out
        // a layer of noise:
        {
            let res = crate::with_retry("http_version_fetch", || async {
                match ehttp::fetch_async(ehttp::Request::get(format!(
                    "{}/version",
                    origin.as_url()
                )))
                .await
                {
                    Ok(res) => Ok(res),
                    Err(err) => {
                        let mut msg = format!("failed to connect to server '{origin}': {err}");
                        if let Some(suggested) = suggest_api_prefix(&origin) {
                            msg.push_str(&format!(". Did you mean '{suggested}'?"));
                        }
                        Err(ApiError::connection(msg))
                    }
                }
            })
            .await?;

            if !res.ok {
                let hint = suggest_api_prefix(&origin).map(|suggested| {
                    format!(
                        "Did you mean '{suggested}'? Rerun Cloud endpoints require the 'api.' prefix"
                    )
                });
                return Err(ApiError::invalid_server(origin.clone(), hint.as_deref()));
            }
        }

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
                if let Some(credentials) = credentials {
                    Err(ApiError::credentials_with_source(
                        ClientCredentialsError::UnauthenticatedBadToken {
                            status: err.into(),
                            credentials,
                        },
                        "verifying connection to server",
                    ))
                } else {
                    Err(ApiError::credentials_with_source(
                        ClientCredentialsError::UnauthenticatedMissingToken(err.into()),
                        "verifying connection to server",
                    ))
                }
            }

            Err(err) => {
                if let Some(cred_error) = err.source().and_then(|s| {
                    s.downcast_ref::<re_auth::credentials::CredentialsProviderError>()
                }) {
                    match cred_error {
                        CredentialsProviderError::SessionExpired => {
                            Err(ApiError::credentials_with_source(
                                ClientCredentialsError::SessionExpired,
                                "session expired",
                            ))
                        }
                        CredentialsProviderError::Custom(_) => {
                            Err(ApiError::credentials_with_source(
                                ClientCredentialsError::RefreshError(err.into()),
                                "refreshing credentials",
                            ))
                        }
                    }
                } else {
                    Err(ApiError::tonic(err, "verifying connection to server"))
                }
            }

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
