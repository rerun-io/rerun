//! Bind a registry to one server origin.

use crate::{ApiResult, Connection, ConnectionClient, ConnectionRegistryHandle};

/// Access to the connection and credentials of one origin.
#[derive(Clone)]
pub struct ConnectionHandle {
    origin: re_uri::Origin,
    registry: ConnectionRegistryHandle,
}

impl ConnectionHandle {
    pub fn new(registry: ConnectionRegistryHandle, origin: re_uri::Origin) -> Self {
        Self { origin, registry }
    }

    pub async fn connection(&self) -> ApiResult<Connection> {
        self.registry.connection(self.origin.clone()).await
    }

    pub async fn client(&self) -> ApiResult<ConnectionClient> {
        Ok(self.connection().await?.client)
    }

    pub fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }

    pub fn connection_registry(&self) -> &ConnectionRegistryHandle {
        &self.registry
    }
}

impl std::fmt::Debug for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionHandle")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}
