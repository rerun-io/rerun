use ahash::HashMap;
use tokio_stream::StreamExt as _;

use re_grpc_client::redap::ConnectionError;
use re_grpc_client::{redap, StreamError};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_encoding::codec::CodecError;
use re_protos::catalog::v1alpha1::ext::{DatasetEntry, EntryDetails};
use re_protos::catalog::v1alpha1::{
    EntryFilter, EntryKind, FindEntriesRequest, ReadDatasetEntryRequest,
};
use re_protos::common::v1alpha1::ext::{DatasetHandle, EntryId};
use re_protos::manifest_registry::v1alpha1::ListPartitionsRequest;
use re_protos::TypeConversionError;
use re_sorbet::{BatchType, SorbetBatch, SorbetError};
use re_viewer_context::AsyncRuntimeHandle;

use crate::context::Context;
use crate::requested_object::RequestedObject;

#[derive(Debug, thiserror::Error)]
pub enum EntryError {
    #[error(transparent)]
    TonicError(#[from] tonic::Status),

    #[error(transparent)]
    ConnectionError(#[from] ConnectionError),

    #[error(transparent)]
    StreamError(#[from] StreamError),

    #[error(transparent)]
    TypeConversionError(#[from] TypeConversionError),

    #[error(transparent)]
    CodecError(#[from] CodecError),

    #[error(transparent)]
    SorbetError(#[from] SorbetError),

    #[error("Field `{0}` not set")]
    FieldNotSet(&'static str),
}

pub struct Dataset {
    pub entry_details: EntryDetails,

    pub origin: re_uri::Origin,

    pub partition_table: Vec<SorbetBatch>,
}

impl Dataset {
    pub fn id(&self) -> EntryId {
        self.entry_details.id
    }
}

pub struct Entries {
    //TODO(ab): in the future, there will be more kinds of entries
    datasets: RequestedObject<Result<HashMap<EntryId, Dataset>, EntryError>>,
}

impl Entries {
    pub fn new(
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
    ) -> Self {
        let datasets = find_dataset_entries(origin.clone());

        Self {
            datasets: RequestedObject::new_with_repaint(runtime, egui_ctx.clone(), datasets),
        }
    }

    pub fn on_frame_start(&mut self) {
        self.datasets.on_frame_start();
    }

    pub fn find_dataset(&self, entry_id: EntryId) -> Option<&Dataset> {
        self.datasets.try_as_ref()?.as_ref().ok()?.get(&entry_id)
    }

    pub fn find_dataset_by_name(&self, dataset_name: &str) -> Option<&Dataset> {
        self.datasets
            .try_as_ref()?
            .as_ref()
            .ok()?
            .values()
            .find(|dataset| dataset.entry_details.name == dataset_name)
    }

    /// [`list_item::ListItem`]-based UI for the collections.
    pub fn panel_ui(&self, _ctx: &Context<'_>, _ui: &mut egui::Ui) {
        //TODO

        // for collection in self.datasets.values() {
        //     match collection.try_as_ref() {
        //         None => {
        //             ui.list_item_flat_noninteractive(
        //                 list_item::LabelContent::new("Loading default collection…").italics(true),
        //             );
        //         }
        //
        //         Some(Ok(collection)) => {
        //             let is_selected = *ctx.selected_collection == Some(collection.dataset_id);
        //
        //             let content = list_item::LabelContent::new(&collection.name);
        //             let response = ui.list_item().selected(is_selected).show_flat(ui, content);
        //
        //             if response.clicked() {
        //                 let _ = ctx
        //                     .command_sender
        //                     .send(Command::SelectCollection(collection.dataset_id));
        //             }
        //         }
        //
        //         Some(Err(err)) => {
        //             ui.list_item_flat_noninteractive(list_item::LabelContent::new(
        //                 egui::RichText::new("Failed to load").color(ui.visuals().error_fg_color),
        //             ))
        //                 .on_hover_text(err.to_string());
        //         }
        //     }
        // }
    }
}

async fn find_dataset_entries(
    origin: re_uri::Origin,
) -> Result<HashMap<EntryId, Dataset>, EntryError> {
    let mut client = redap::catalog_client(origin.clone()).await?;

    let resp = client
        .find_entries(FindEntriesRequest {
            filter: Some(EntryFilter {
                id: None,
                name: None,
                entry_kind: Some(EntryKind::Dataset.into()),
            }),
        })
        .await?
        .into_inner();

    let mut datasets = HashMap::default();

    for entry_details in resp.entries {
        let entry_details = EntryDetails::try_from(entry_details)?;

        let dataset_entry: DatasetEntry = client
            .read_dataset_entry(ReadDatasetEntryRequest {
                id: Some(entry_details.id.into()),
            })
            .await?
            .into_inner()
            .dataset
            .ok_or(EntryError::FieldNotSet("dataset"))?
            .try_into()?;

        let partition_table =
            stream_partition_table(origin.clone(), dataset_entry.handle.clone().into()).await?;

        let entry = Dataset {
            entry_details,
            origin: origin.clone(),
            partition_table,
        };

        datasets.insert(entry.entry_details.id, entry);
    }

    Ok(datasets)
}

async fn stream_partition_table(
    origin: re_uri::Origin,
    dataset_handle: DatasetHandle,
) -> Result<Vec<SorbetBatch>, EntryError> {
    let mut client = redap::manifest_registry_client(origin).await?;

    let mut response = client
        .list_partitions(ListPartitionsRequest {
            entry: Some(dataset_handle.into()),
            scan_parameters: None,
        })
        .await?
        .into_inner();

    let mut sorbet_batches = Vec::new();

    while let Some(result) = response.next().await {
        let record_batch = result?
            .data
            .ok_or(EntryError::FieldNotSet("data"))?
            .decode()?;

        let sorbet_batch = SorbetBatch::try_from_record_batch(&record_batch, BatchType::Dataframe)?;

        sorbet_batches.push(sorbet_batch);
    }

    Ok(sorbet_batches)
}
