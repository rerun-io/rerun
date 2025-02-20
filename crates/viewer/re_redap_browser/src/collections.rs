use std::sync::Arc;

use ahash::HashMap;
use parking_lot::Mutex;
use tokio_stream::StreamExt as _;

use re_grpc_client::{redap, StreamError, TonicStatusError};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::remote_store::v0::QueryCatalogRequest;
use re_sorbet::{BatchType, SorbetBatch};
use re_ui::{list_item, UiExt};
use re_viewer_context::AsyncRuntimeHandle;

use crate::context::Context;
use crate::servers::Command;

/// An id for a [`Collection`].
/// //TODO(ab): this should be a properly defined id provided by the redap server
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CollectionId(pub egui::Id);

/// An individual collection of recordings within a catalog.
pub struct Collection {
    pub collection_id: CollectionId,

    pub name: String,

    pub collection: Vec<SorbetBatch>,
}

/// A handle on an in-flight collection query. Contains `Some(Ok(_))` or `Some(Err(_))` once the
/// query has completed.
struct CollectionQueryHandle {
    result: Arc<Mutex<Option<Result<Collection, StreamError>>>>,
}

impl CollectionQueryHandle {
    /// Initiate a collection query call.
    pub fn new(runtime: &AsyncRuntimeHandle, origin: re_uri::Origin) -> Self {
        let result = Arc::new(Mutex::new(None));
        let handle = Self {
            result: result.clone(),
        };

        runtime.spawn_future(async move {
            let collection = stream_catalog_async(origin.clone()).await;
            result.lock().replace(collection);
        });

        handle
    }
}

/// Either a [`Collection`] or a handle on the query to get it.
enum CollectionOrQueryHandle {
    QueryHandle(CollectionQueryHandle),
    Collection(Result<Collection, StreamError>),
}

/// A collection of [`Collection`]s.
#[derive(Default)]
pub struct Collections {
    collections: HashMap<re_uri::Origin, CollectionOrQueryHandle>,
}

impl Collections {
    pub fn add(&mut self, runtime: &AsyncRuntimeHandle, origin: re_uri::Origin) {
        //TODO(ab): should we return error if the requested collection already exists? Or maybe just
        // query it again.
        self.collections.entry(origin.clone()).or_insert_with(|| {
            CollectionOrQueryHandle::QueryHandle(CollectionQueryHandle::new(runtime, origin))
        });
    }

    /// Convert all completed queries into proper collections.
    pub fn on_frame_start(&mut self) {
        for collection in self.collections.values_mut() {
            let result = match collection {
                CollectionOrQueryHandle::QueryHandle(handle) => handle.result.lock().take(),
                CollectionOrQueryHandle::Collection(_) => None,
            };

            if let Some(result) = result {
                *collection = CollectionOrQueryHandle::Collection(result);
            }
        }
    }

    /// Find a [`Collection`] with the given [`CollectionId`].
    pub fn find(&self, collection_id: CollectionId) -> Option<&Collection> {
        self.collections
            .values()
            .filter_map(|handle| match handle {
                CollectionOrQueryHandle::QueryHandle(_) => None,
                CollectionOrQueryHandle::Collection(collection) => collection.as_ref().ok(),
            })
            .find(|collection| collection.collection_id == collection_id)
    }

    /// [`list_item::ListItem`]-based UI for the collections.
    pub fn panel_ui(&self, ctx: &Context<'_>, ui: &mut egui::Ui) {
        for collection in self.collections.values() {
            match collection {
                CollectionOrQueryHandle::QueryHandle(_) => {
                    ui.list_item_flat_noninteractive(
                        list_item::LabelContent::new("Loading default collection…").italics(true),
                    );
                }
                CollectionOrQueryHandle::Collection(Ok(collection)) => {
                    let is_selected = *ctx.selected_collection == Some(collection.collection_id);

                    let content = list_item::LabelContent::new(&collection.name);
                    let response = ui.list_item().selected(is_selected).show_flat(ui, content);

                    if response.clicked() {
                        let _ = ctx
                            .command_sender
                            .send(Command::SelectCollection(collection.collection_id));
                    }
                }
                CollectionOrQueryHandle::Collection(Err(err)) => {
                    ui.list_item_flat_noninteractive(list_item::LabelContent::new(
                        egui::RichText::new("Failed to load").color(ui.visuals().error_fg_color),
                    ))
                    .on_hover_text(err.to_string());
                }
            }
        }
    }
}

async fn stream_catalog_async(origin: re_uri::Origin) -> Result<Collection, StreamError> {
    let mut client = redap::client(origin.clone()).await?;

    re_log::debug!("Fetching collection…");

    let catalog_query_response = client
        .query_catalog(QueryCatalogRequest {
            column_projection: None, // fetch all columns
            filter: None,            // fetch all rows
        })
        .await
        .map_err(TonicStatusError)?;

    let sorbet_batches = catalog_query_response
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
        .map(|record_batch| {
            record_batch.and_then(|record_batch| {
                SorbetBatch::try_from_record_batch(&record_batch, BatchType::Dataframe)
                    .map_err(Into::into)
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .await?;

    //TODO(ab): ideally this is provided by the server
    let collection_id =
        CollectionId(egui::Id::new(origin.clone()).with("__top_level_collection__"));
    let collection = Collection {
        collection_id,
        //TODO(ab): this should be provided by the server
        name: "default".to_owned(),
        collection: sorbet_batches,
    };

    Ok(collection)
}
