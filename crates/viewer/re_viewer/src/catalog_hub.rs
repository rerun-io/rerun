use std::sync::Arc;

use ahash::HashMap;
use parking_lot::Mutex;
use tokio_stream::StreamExt as _;
use url::Url;

use re_grpc_client::{StreamError, TonicStatusError};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::remote_store::v0::{storage_node_client::StorageNodeClient, QueryCatalogRequest};

use crate::AsyncRuntimeHandle;

/// An individual catalog.
pub struct Catalog {
    data: Vec<re_chunk::TransportChunk>, // TODO: transport chunk is going away.
}

/// All catalogs known to the viewer.
#[derive(Default)]
pub struct CatalogHub {
    catalogs: Arc<Mutex<HashMap<Url, Catalog>>>,
    // TODO(andreas,antoine): Keep track of in-flight requests.
    //in_flight_requests: HashMap<Uri, Future<Result<Catalog, Error>>>,
}

impl CatalogHub {
    /// Asynchronously fetches a catalog from a URL and adds it to the hub.
    ///
    /// If this url was used before, it will refresh the existing catalog in the hub.
    pub fn fetch_catalog(&self, runtime: &AsyncRuntimeHandle, redap_endpoint: Url) {
        // TODO: app should handle this and be careful about existing runtimes.
        let _ = tokio::runtime::Runtime::new();

        let catalogs = self.catalogs.clone();
        runtime.spawn_future(async move {
            let result = stream_catalog_async(redap_endpoint, catalogs).await;
            if let Err(e) = result {
                // TODO(andreas,antoine): Surface this in the UI in a better way.
                re_log::error!("Failed to fetch catalog: {e}");
            }
        });
    }
}

async fn stream_catalog_async(
    redap_endpoint: Url,
    catalogs: Arc<Mutex<HashMap<Url, Catalog>>>,
) -> Result<(), StreamError> {
    let mut client = {
        #[cfg(target_arch = "wasm32")]
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            redap_endpoint.to_string(),
            tonic_web_wasm_client::options::FetchOptions::new(),
        );

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = tonic::transport::Endpoint::new(redap_endpoint.to_string())?
            .connect()
            .await?;

        StorageNodeClient::new(tonic_client)
    };

    re_log::debug!("Fetching catalog…");

    let catalog_query_response = client
        .query_catalog(QueryCatalogRequest {
            column_projection: None, // fetch all columns
            filter: None,            // fetch all rows
        })
        .await
        .map_err(TonicStatusError)?;

    let chunks = catalog_query_response
        .into_inner()
        .map(|streaming_result| {
            streaming_result
                .and_then(|result| {
                    result
                        .decode()
                        .map_err(|err| tonic::Status::internal(err.to_string()))
                })
                .map_err(TonicStatusError)
                .map(re_chunk::TransportChunk::from)
        })
        .collect::<Result<Vec<_>, _>>()
        .await?;

    let catalog = Catalog { data: chunks };
    let previous_catalog = catalogs.lock().insert(redap_endpoint.clone(), catalog);
    if previous_catalog.is_some() {
        re_log::debug!("Updated catalog for {}.", redap_endpoint.to_string());
    } else {
        re_log::debug!("Fetched new catalog for {}.", redap_endpoint.to_string());
    }

    Ok(())
}
