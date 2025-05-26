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

/// TODO: cheap to clone
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
        let token = inner.tokens.get(&origin).cloned();

        //TODO: move env var logic here

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
