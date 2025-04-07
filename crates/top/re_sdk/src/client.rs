use std::{net::Ipv4Addr, str::FromStr as _};

use re_grpc_client::message_proxy::TableClient;
use re_log_types::{TableId, TableMsg};
use re_uri::{Origin, ProxyUri};
use url::Host;

/// A builder for [`ViewerClient`].
pub struct ViewerClientBuilder {
    endpoint: ProxyUri,
}

impl Default for ViewerClientBuilder {
    fn default() -> Self {
        Self {
            endpoint: ProxyUri {
                origin: Origin {
                    scheme: re_uri::Scheme::RerunHttp,
                    host: Host::Ipv4(Ipv4Addr::new(0, 0, 0, 0)),
                    port: 9876,
                },
            },
        }
    }
}

impl ViewerClientBuilder {
    /// Connects to the viewer and creates a handle for it.
    pub fn connect(self) -> ViewerClient {
        ViewerClient {
            client: TableClient::new(self.endpoint),
        }
    }

    /// The url of the Rerun viewer.
    pub fn with_url(mut self, url: impl AsRef<str>) -> Result<Self, re_uri::Error> {
        let origin = Origin::from_str(url.as_ref())?;
        self.endpoint = ProxyUri { origin };
        Ok(self)
    }
}

/// Create a connection to an instance of a Rerun viewer.
pub struct ViewerClient {
    client: TableClient,
}

impl Default for ViewerClient {
    fn default() -> Self {
        Self::builder().connect()
    }
}

impl ViewerClient {
    /// Creates a builder for the client.
    pub fn builder() -> ViewerClientBuilder {
        Default::default()
    }

    /// Sends a table to the viewer. The input can be arbitrary Arrow record batches.
    pub fn send_table(&self, id: impl Into<String>, dataframe: arrow::array::RecordBatch) {
        self.client.send_msg(TableMsg {
            id: TableId::from(id.into()),
            data: dataframe,
        });
    }
}
