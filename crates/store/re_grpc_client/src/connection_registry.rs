use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use re_auth::Jwt;

use crate::redap::{ConnectionError, RedapClient};

#[derive(Default)]
struct ConnectionRegistryImpl {
    tokens: HashMap<re_uri::Origin, Jwt>,
}

/// TODO: cheap to clone
#[derive(Default, Clone)]
pub struct ConnectionRegistry {
    inner: Arc<RwLock<ConnectionRegistryImpl>>,
}

impl ConnectionRegistry {
    pub fn set_token(&self, origin: re_uri::Origin, token: Jwt) {
        let mut inner = self.inner.blocking_write();
        inner.tokens.insert(origin, token);
    }

    pub async fn client(&self, origin: re_uri::Origin) -> Result<RedapClient, ConnectionError> {
        let inner = self.inner.read().await;
        let token = inner.tokens.get(&origin).cloned();

        //TODO: move env var logic here

        //TODO: should I keep it around and clone instead?
        crate::redap::client(origin, token).await
    }
}
