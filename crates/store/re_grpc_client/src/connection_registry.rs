use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use tokio::sync::RwLock;

use re_auth::Jwt;

use crate::redap::{ConnectionError, RedapClient};

#[derive(Default)]
struct ConnectionRegistryImpl {
    /// The available authentication tokens.
    tokens: HashMap<re_uri::Origin, Jwt>,

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

/// Registry of all tokens and connections to the redap servers.
///
/// This registry is cheap to clone.
#[derive(Default, Clone)]
pub struct ConnectionRegistry {
    inner: Arc<RwLock<ConnectionRegistryImpl>>,
}

impl ConnectionRegistry {
    pub fn set_token(&self, origin: &re_uri::Origin, token: Jwt) {
        let mut inner = self.inner.blocking_write();
        inner.tokens.insert(origin.clone(), token);
        inner.clients.remove(origin);
    }

    pub fn set_fallback_token(&self, token: Jwt) {
        let mut inner = self.inner.blocking_write();
        inner.fallback_token = Some(token);
    }

    /// Get a client for the given origin, creating one if it doesn't exist yet.
    ///
    /// If a token has already been registered for this origin, it will be used. It will attempt to
    /// use the following token, in this order:
    /// - The fallback token, if set via [`Self::set_fallback_token`].
    /// - The `REDAP_TOKEN` environment variable is set.
    ///
    /// Failing that, no token will be used.
    ///
    /// Note that a token set via `REDAP_TOKEN` will not be persisted unless [`Self::set_token`] is
    /// explicitly called. The rationale is to avoid sneakily saving in clear text potentially
    /// sensitive information.
    pub async fn client(&self, origin: re_uri::Origin) -> Result<RedapClient, ConnectionError> {
        let inner = self.inner.read().await;
        if let Some(client) = inner.clients.get(&origin) {
            return Ok(client.clone());
        }
        drop(inner);

        let mut inner = self.inner.write().await;
        let token = inner
            .tokens
            .get(&origin)
            .cloned()
            .or_else(|| inner.fallback_token.clone())
            .or_else(get_token_from_env);

        let client = crate::redap::client(origin.clone(), token).await;
        match client {
            Ok(client) => {
                inner.clients.insert(origin, client.clone());
                Ok(client)
            }
            Err(err) => Err(err),
        }
    }

    /// Dump all tokens for persistence purposes.
    pub fn dump_tokens(&self) -> SerializedTokens {
        SerializedTokens(
            self.inner
                .blocking_read()
                .tokens
                .iter()
                .map(|(origin, token)| (origin.clone(), token.to_string()))
                .collect(),
        )
    }

    /// Load tokens from persistence.
    ///
    /// IMPORTANT: This will NOT overwrite any existing tokens, since it is assumed that existing
    /// tokens were explicitly set by the user (e.g. with `--token`).
    pub fn load_tokens(&self, tokens: SerializedTokens) {
        let mut inner = self.inner.blocking_write();
        for (origin, token) in tokens.0 {
            if let Entry::Vacant(e) = inner.tokens.entry(origin.clone()) {
                if let Ok(jwt) = Jwt::try_from(token) {
                    e.insert(jwt);
                } else {
                    re_log::debug!("Failed to parse token for origin {origin}");
                }
            } else {
                re_log::trace!("Ignoring token for origin {origin} as it is already set");
            }
        }
    }
}

// ---

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializedTokens(Vec<(re_uri::Origin, String)>);

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
