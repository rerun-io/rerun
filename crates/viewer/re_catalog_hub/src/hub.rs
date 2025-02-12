use std::sync::Arc;

use ahash::HashMap;
use arrow::array::RecordBatch;
use parking_lot::Mutex;
use tokio_stream::StreamExt as _;

use re_grpc_client::{redap, StreamError, TonicStatusError};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::remote_store::v0::{storage_node_client::StorageNodeClient, QueryCatalogRequest};
use re_ui::{list_item, UiExt};
use re_viewer_context::{AsyncRuntimeHandle, ViewerContext};

pub struct CollectionHandle {
    server_origin: redap::Origin,
    collection_index: usize,
}

/// An individual catalog.
pub struct Catalog {
    collections: Vec<RecordingCollection>,
}

impl Catalog {
    fn is_empty(&self) -> bool {
        self.collections.is_empty()
    }
}

/// An individual collection of recordings within a catalog.
pub struct RecordingCollection {
    // TODO: other information.
    // TODO: transport chunk is going away.
    pub collection: Vec<RecordBatch>,
}

/// All catalogs known to the viewer.
// TODO(andreas,antoine): Eventually, collections are part of a catalog, meaning there is going to be multiple ones.
#[derive(Default)]
pub struct CatalogHub {
    // TODO(andreas,antoine): One of those Urls is probably going to be a local catalog.
    catalogs: Arc<Mutex<HashMap<redap::Origin, Catalog>>>,
    // TODO(andreas,antoine): Keep track of in-flight requests.
    //in_flight_requests: HashMap<Uri, Future<Result<RecordingCollection, Error>>>,
    selected_collection: Option<CollectionHandle>,

    command_queue: Arc<Mutex<Vec<Command>>>,
}

pub enum Command {
    SelectCollection(CollectionHandle),
    DeselectCollection,
}

impl CatalogHub {
    /// Asynchronously fetches a catalog from a URL and adds it to the hub.
    ///
    /// If this url was used before, it will refresh the existing catalog in the hub.
    pub fn fetch_catalog(&self, runtime: &AsyncRuntimeHandle, redap_endpoint: redap::Origin) {
        let catalogs = self.catalogs.clone();
        runtime.spawn_future(async move {
            let result = stream_catalog_async(redap_endpoint, catalogs).await;
            if let Err(e) = result {
                // TODO(andreas,antoine): Surface this in the UI in a better way.
                re_log::error!("Failed to fetch catalog: {e}");
            }
        });
    }

    /// Process any pending commands
    pub fn on_frame_start(&mut self) {
        for command in self.command_queue.lock().drain(..) {
            match command {
                Command::SelectCollection(collection_handle) => {
                    self.selected_collection = Some(collection_handle);
                }

                Command::DeselectCollection => self.selected_collection = None,
            }
        }
    }

    pub fn server_panel_ui(&self, ui: &mut egui::Ui) {
        ui.panel_content(|ui| {
            ui.panel_title_bar(
                "Servers",
                Some("These are the currently connected Redap servers."),
            );
        });

        egui::ScrollArea::both()
            .id_salt("servers_scroll_area")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.panel_content(|ui| {
                    re_ui::list_item::list_item_scope(ui, "server panel", |ui| {
                        self.server_list_ui(ui);
                    });
                });
            });
    }

    pub fn is_empty(&self) -> bool {
        self.catalogs.lock().is_empty()
    }

    pub fn is_collection_selected(&self) -> bool {
        self.selected_collection
            .as_ref()
            .map(|handle| self.validate_handle(handle))
            .unwrap_or(false)
    }

    fn validate_handle(&self, handle: &CollectionHandle) -> bool {
        let catalogs = self.catalogs.lock();
        if let Some(catalog) = catalogs.get(&handle.server_origin) {
            return catalog.collections.get(handle.collection_index).is_some();
        }

        false
    }

    pub fn server_list_ui(&self, ui: &mut egui::Ui) {
        for (origin, catalog) in self.catalogs.lock().iter() {
            let content = list_item::LabelContent::new(origin.to_http_scheme());
            ui.list_item()
                .interactive(false)
                .show_hierarchical_with_children(
                    ui,
                    egui::Id::new(origin).with("server_item"),
                    true,
                    content,
                    |ui| {
                        self.catalog_list_ui(ui, origin, catalog);
                    },
                );
        }
    }

    fn catalog_list_ui(&self, ui: &mut egui::Ui, origin: &redap::Origin, catalog: &Catalog) {
        if catalog.is_empty() {
            ui.list_item_flat_noninteractive(list_item::LabelContent::new("(empty)").italics(true));
        } else {
            for (index, _collection) in catalog.collections.iter().enumerate() {
                let is_selected =
                    if let Some(selected_collection) = self.selected_collection.as_ref() {
                        selected_collection.server_origin == *origin
                            && selected_collection.collection_index == index
                    } else {
                        false
                    };

                let content = list_item::LabelContent::new(format!("collection {index}"));
                let response = ui.list_item().selected(is_selected).show_flat(ui, content);

                if response.clicked() {
                    self.command_queue
                        .lock()
                        .push(Command::SelectCollection(CollectionHandle {
                            server_origin: origin.clone(),
                            collection_index: index,
                        }));
                }
            }
        }
    }

    pub fn selected_collection_ui(&self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        if let Some(selected_collection) = self.selected_collection.as_ref() {
            let catalogs = self.catalogs.lock();
            if let Some(catalog) = catalogs.get(&selected_collection.server_origin) {
                if let Some(collection) = catalog
                    .collections
                    .get(selected_collection.collection_index)
                {
                    let mut commands = super::collection_ui::collection_ui(ctx, ui, collection);
                    if !commands.is_empty() {
                        self.command_queue.lock().extend(commands.drain(..));
                    }
                }
            }
        }
    }
}

async fn stream_catalog_async(
    redap_endpoint: redap::Origin,
    catalogs: Arc<Mutex<HashMap<redap::Origin, Catalog>>>,
) -> Result<(), StreamError> {
    let mut client = {
        #[cfg(target_arch = "wasm32")]
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            redap_endpoint.to_string(),
            tonic_web_wasm_client::options::FetchOptions::new(),
        );

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = tonic::transport::Endpoint::new(redap_endpoint.to_http_scheme())?
            .tls_config(tonic::transport::ClientTlsConfig::new().with_enabled_roots())?
            .connect()
            .await?;

        StorageNodeClient::new(tonic_client)
    };

    re_log::debug!("Fetching collection…");

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
                .map_err(StreamError::from)
        })
        .collect::<Result<Vec<_>, _>>()
        .await?;

    let catalog = Catalog {
        collections: vec![RecordingCollection { collection: chunks }],
    };

    let previous_catalog = catalogs.lock().insert(redap_endpoint.clone(), catalog);
    if previous_catalog.is_some() {
        re_log::debug!("Updated catalog for {}.", redap_endpoint.to_string());
    } else {
        re_log::debug!("Fetched new catalog for {}.", redap_endpoint.to_string());
    }

    Ok(())
}
