use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use re_auth::Jwt;

use crate::redap::{ConnectionError, RedapClient};

#[derive(Default)]
struct ConnectionRegistryImpl {
    /// The available authentication tokens.
    tokens: HashMap<re_uri::Origin, Jwt>,

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

    pub async fn client(&self, origin: re_uri::Origin) -> Result<RedapClient, ConnectionError> {
        let inner = self.inner.read().await;
        if let Some(client) = inner.clients.get(&origin) {
            return Ok(client.clone());
        }

        let mut inner = self.inner.write().await;
        let token = inner
            .tokens
            .get(&origin)
            .cloned()
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
}

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
