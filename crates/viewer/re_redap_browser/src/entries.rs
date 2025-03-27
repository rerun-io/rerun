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
use re_protos::common::v1alpha1::ext::EntryId;
use re_protos::frontend::v1alpha1::ListPartitionsRequest;
use re_protos::TypeConversionError;
use re_sorbet::{BatchType, SorbetBatch, SorbetError};
use re_ui::{icons, list_item, UiExt as _};
use re_viewer_context::AsyncRuntimeHandle;

use crate::context::Context;
use crate::requested_object::RequestedObject;
use crate::servers::Command;

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
    pub dataset_entry: DatasetEntry,

    pub origin: re_uri::Origin,

    pub partition_table: Vec<SorbetBatch>,
}

impl Dataset {
    pub fn id(&self) -> EntryId {
        self.dataset_entry.details.id
    }

    pub fn name(&self) -> &str {
        self.dataset_entry.details.name.as_ref()
    }
}

/// All the entries of a server.
pub struct Entries {
    //TODO(ab): in the future, there will be more kinds of entries

    // TODO(ab): we currently load the ENTIRE list of datasets, including their partition tables. We
    // will need to be more granular about this in the future.
    datasets: RequestedObject<Result<HashMap<EntryId, Dataset>, EntryError>>,
}

impl Entries {
    pub fn new(
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
    ) -> Self {
        let datasets = fetch_dataset_entries(origin);

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
            .find(|dataset| dataset.name() == dataset_name)
    }

    /// [`list_item::ListItem`]-based UI for the datasets.
    pub fn panel_ui(&self, ctx: &Context<'_>, ui: &mut egui::Ui) {
        match self.datasets.try_as_ref() {
            None => {
                ui.list_item_flat_noninteractive(
                    list_item::LabelContent::new("Loading datasets…").italics(true),
                );
            }

            Some(Ok(datasets)) => {
                for dataset in datasets.values() {
                    let is_selected = ctx.is_selected(dataset.id());

                    let content =
                        list_item::LabelContent::new(dataset.name()).with_icon(&icons::DATASET);
                    let response = ui.list_item().selected(is_selected).show_flat(ui, content);

                    if response.clicked() {
                        let _ = ctx.command_sender.send(Command::SelectEntry(dataset.id()));
                    }
                }
            }

            Some(Err(err)) => {
                ui.list_item_flat_noninteractive(list_item::LabelContent::new(
                    egui::RichText::new("Failed to load datasets")
                        .color(ui.visuals().error_fg_color),
                ))
                .on_hover_text(err.to_string());
            }
        }
    }
}

async fn fetch_dataset_entries(
    origin: re_uri::Origin,
) -> Result<HashMap<EntryId, Dataset>, EntryError> {
    let mut client = redap::client(origin.clone()).await?;

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

        let partition_table = fetch_partition_table(&mut client, entry_details.id).await?;

        let entry = Dataset {
            dataset_entry,
            origin: origin.clone(),
            partition_table,
        };

        datasets.insert(entry.id(), entry);
    }

    Ok(datasets)
}

async fn fetch_partition_table(
    client: &mut redap::Client,
    entry_id: EntryId,
) -> Result<Vec<SorbetBatch>, EntryError> {
    let mut response = client
        .list_partitions(ListPartitionsRequest {
            dataset_id: Some(entry_id.into()),
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
