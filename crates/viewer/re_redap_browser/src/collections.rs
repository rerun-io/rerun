use ahash::HashMap;
use tokio_stream::StreamExt as _;

use re_grpc_client::{redap, StreamError, TonicStatusError};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::remote_store::v1alpha1::{CatalogEntry, QueryCatalogRequest};
use re_sorbet::{BatchType, SorbetBatch};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::AsyncRuntimeHandle;

use crate::context::Context;
use crate::requested_object::RequestedObject;
use crate::servers::Command;

/// An id for a [`Collection`].
/// //TODO(ab): this should be a properly defined id provided by the redap server
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CollectionId(egui::Id);

impl From<&re_uri::Origin> for CollectionId {
    fn from(origin: &re_uri::Origin) -> Self {
        Self(egui::Id::new(origin.clone()).with("__top_level_collection__"))
    }
}

/// An individual collection of recordings within a catalog.
pub struct Collection {
    pub collection_id: CollectionId,

    pub name: String,

    pub collection: Vec<SorbetBatch>,
}

/// A collection of [`Collection`]s.
#[derive(Default)]
pub struct Collections {
    //TODO(ab): these should be indexed by collection id
    collections: HashMap<re_uri::Origin, RequestedObject<Result<Collection, StreamError>>>,
}

impl Collections {
    pub fn fetch(
        &mut self,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
    ) {
        self.collections.insert(
            origin.clone(),
            RequestedObject::new_with_repaint(
                runtime,
                egui_ctx.clone(),
                stream_catalog_async(origin),
            ),
        );
    }

    /// Convert all completed queries into proper collections.
    pub fn on_frame_start(&mut self) {
        for collection in self.collections.values_mut() {
            collection.on_frame_start();
        }
    }

    /// Find a [`Collection`] with the given [`CollectionId`].
    pub fn find(&self, collection_id: CollectionId) -> Option<&Collection> {
        self.collections
            .values()
            .filter_map(|handle| handle.try_as_ref())
            .filter_map(|result| result.as_ref().ok())
            .find(|collection| collection.collection_id == collection_id)
    }

    /// [`list_item::ListItem`]-based UI for the collections.
    pub fn panel_ui(&self, ctx: &Context<'_>, ui: &mut egui::Ui) {
        for collection in self.collections.values() {
            match collection.try_as_ref() {
                None => {
                    ui.list_item_flat_noninteractive(
                        list_item::LabelContent::new("Loading default collection…").italics(true),
                    );
                }

                Some(Ok(collection)) => {
                    let is_selected = *ctx.selected_collection == Some(collection.collection_id);

                    let content = list_item::LabelContent::new(&collection.name);
                    let response = ui.list_item().selected(is_selected).show_flat(ui, content);

                    if response.clicked() {
                        let _ = ctx
                            .command_sender
                            .send(Command::SelectCollection(collection.collection_id));
                    }
                }

                Some(Err(err)) => {
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
            entry: Some(CatalogEntry {
                name: "default".to_owned(), /* TODO(zehiko) 9116 */
            }),
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
                        .data
                        .ok_or_else(|| {
                            tonic::Status::internal("missing DataframePart in QueryCatalogResponse")
                        })?
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
    let collection_id = CollectionId::from(&origin);
    let collection = Collection {
        collection_id,
        //TODO(ab): this should be provided by the server
        name: "default".to_owned(),
        collection: sorbet_batches,
    };

    Ok(collection)
}
