use std::sync::Arc;

use ahash::HashMap;
use parking_lot::Mutex;
use tokio_stream::StreamExt as _;
use url::Url;

use re_grpc_client::{StreamError, TonicStatusError};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::remote_store::v0::{storage_node_client::StorageNodeClient, QueryCatalogRequest};

/// An individual catalog.
pub struct Catalog {
    data: re_chunk::TransportChunk,
}

/// All catalogs known to the viewer.
#[derive(Default)]
pub struct CatalogHub {
    catalogs: Arc<Mutex<HashMap<Url, Catalog>>>,
    //in_flight_requests: HashMap<Uri, Future<Result<Catalog, Error>>>,
}

impl CatalogHub {
    /// Asynchronously fetches a catalog from a URL and adds it to the hub.
    ///
    /// If this url was used before, it will refresh the existing catalog in the hub.
    pub fn fetch_catalog(&mut self, redap_endpoint: Url) {
        // TODO: app should handle this and be careful about existing runtimes.
        let _ = tokio::runtime::Runtime::new();

        let catalogs = self.catalogs.clone();
        spawn_future(async move {
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
    re_log::debug!("Connecting to {redap_endpoint}…");

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

    let mut resp = client
        .query_catalog(QueryCatalogRequest {
            column_projection: None, // fetch all columns
            filter: None,            // fetch all rows
        })
        .await
        .map_err(TonicStatusError)?
        .into_inner()
        .map(|resp| {
            resp.and_then(|r| {
                r.decode()
                    .map_err(|err| tonic::Status::internal(err.to_string()))
            })
        });

    // Close connection once we can.
    drop(client);

    // Gather transport chunks from the response.
    while let Some(result) = resp.next().await {
        let transport_chunk = re_chunk::TransportChunk::from(result.map_err(TonicStatusError)?);
        let catalog = Catalog {
            data: transport_chunk,
        };
        catalogs.lock().insert(redap_endpoint, catalog);

        // TODO: can there be many?
        return Ok(());
    }

    // TODO(andreas,antoine): Report that we're done fetching the catalog.

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static + Send,
{
    tokio::spawn(future);
}
