use std::collections::BTreeMap;

use ahash::HashMap;
use tokio_stream::StreamExt as _;

use re_data_ui::item_ui::entity_db_button_ui;
use re_data_ui::DataUi as _;
use re_grpc_client::redap::ConnectionError;
use re_grpc_client::{redap, StreamError};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_encoding::codec::CodecError;
use re_log_types::{ApplicationId, EntryId, StoreKind};
use re_protos::catalog::v1alpha1::{
    ext::{DatasetEntry, EntryDetails},
    EntryFilter, FindEntriesRequest, ReadDatasetEntryRequest,
};
use re_protos::frontend::v1alpha1::ScanPartitionTableRequest;
use re_protos::TypeConversionError;
use re_smart_channel::SmartChannelSource;
use re_sorbet::{BatchType, SorbetBatch, SorbetError};
use re_types::components::Timestamp;
use re_ui::{icons, list_item, UiExt as _, UiLayout};
use re_viewer_context::{
    external::re_entity_db::EntityDb, AsyncRuntimeHandle, DisplayMode, Item, StoreHubEntry,
    SystemCommand, SystemCommandSender as _, ViewerContext,
};

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

    /// [`list_item::ListItem`]-based UI for the datasets.
    pub fn panel_ui(
        &self,
        viewer_context: &ViewerContext<'_>,
        _ctx: &Context<'_>,
        ui: &mut egui::Ui,
        mut recordings: Option<DatasetRecordings<'_>>,
    ) {
        match self.datasets.try_as_ref() {
            None => {
                ui.list_item_flat_noninteractive(
                    list_item::LabelContent::new("Loading datasets…").italics(true),
                );
            }

            Some(Ok(datasets)) => {
                for dataset in datasets.values() {
                    let recordings = recordings
                        .as_mut()
                        .and_then(|r| r.remove(&dataset.id()))
                        .unwrap_or_default();

                    dataset_and_its_recordings_ui(
                        ui,
                        viewer_context,
                        &EntryKind::Remote {
                            origin: dataset.origin.clone(),
                            entry_id: dataset.id(),
                            name: dataset.name().to_owned(),
                        },
                        recordings,
                    );
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

pub type DatasetRecordings<'a> = BTreeMap<EntryId, Vec<&'a EntityDb>>;

pub type RemoteRecordings<'a> = BTreeMap<re_uri::Origin, DatasetRecordings<'a>>;

pub type LocalRecordings<'a> = BTreeMap<ApplicationId, Vec<&'a EntityDb>>;

pub struct SortDatasetsResults<'a> {
    pub remote_recordings: RemoteRecordings<'a>,
    pub example_recordings: LocalRecordings<'a>,
    pub local_recordings: LocalRecordings<'a>,
}

pub fn sort_datasets<'a>(viewer_ctx: &ViewerContext<'a>) -> SortDatasetsResults<'a> {
    let mut remote_recordings: RemoteRecordings<'_> = BTreeMap::new();
    let mut local_recordings: LocalRecordings<'_> = BTreeMap::new();
    let mut example_recordings: LocalRecordings<'_> = BTreeMap::new();

    for entity_db in viewer_ctx
        .storage_context
        .bundle
        .entity_dbs()
        .filter(|r| r.store_kind() == StoreKind::Recording)
    {
        // We want to show all open applications, even if they have no recordings
        let Some(app_id) = entity_db.app_id().cloned() else {
            continue; // this only happens if we haven't even started loading it, or if something is really wrong with it.
        };
        if let Some(SmartChannelSource::RedapGrpcStream(uri)) = &entity_db.data_source {
            let origin_recordings = remote_recordings.entry(uri.origin.clone()).or_default();

            let dataset_recordings = origin_recordings
                // Currently a origin only has a single dataset, this should change soon
                .entry(EntryId::from(uri.dataset_id))
                .or_default();

            dataset_recordings.push(entity_db);
        } else if matches!(&entity_db.data_source, Some(SmartChannelSource::RrdHttpStream {url, ..}) if url.starts_with("https://app.rerun.io"))
        {
            let recordings = example_recordings.entry(app_id).or_default();
            recordings.push(entity_db);
        } else {
            let recordings = local_recordings.entry(app_id).or_default();
            recordings.push(entity_db);
        }
    }

    SortDatasetsResults {
        remote_recordings,
        example_recordings,
        local_recordings,
    }
}

#[derive(Clone, Hash)]
pub enum EntryKind {
    Remote {
        origin: re_uri::Origin,
        entry_id: EntryId,
        name: String,
    },
    Local(ApplicationId),
}

impl EntryKind {
    fn name(&self) -> String {
        match self {
            Self::Remote {
                origin: _,
                entry_id: _,
                name,
            } => name.to_string(),
            Self::Local(app_id) => app_id.to_string(),
        }
    }

    fn select(&self, ctx: &ViewerContext<'_>) {
        ctx.command_sender()
            .send_system(SystemCommand::SetSelection(self.item()));
        match self {
            Self::Remote { entry_id, .. } => {
                ctx.command_sender()
                    .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapEntry(
                        *entry_id,
                    )));
            }
            Self::Local(app) => {
                ctx.command_sender()
                    .send_system(SystemCommand::ActivateApp(app.clone()));
            }
        }
    }

    fn item(&self) -> Item {
        match self {
            Self::Remote {
                name: _,
                origin: _,
                entry_id,
            } => Item::RedapEntry(*entry_id),
            Self::Local(app_id) => Item::AppId(app_id.clone()),
        }
    }

    fn is_active(&self, ctx: &ViewerContext<'_>) -> bool {
        match self {
            Self::Remote { entry_id, .. } => {
                matches!(ctx.global_context.display_mode, DisplayMode::RedapEntry(id) if id == entry_id)
            }
            // TODO(lucasmerlin): Update this when local datasets have a view like remote datasets
            Self::Local(_) => false,
        }
    }

    fn close(&self, ctx: &ViewerContext<'_>, dbs: &Vec<&EntityDb>) {
        match self {
            Self::Remote { .. } => {
                for db in dbs {
                    ctx.command_sender().send_system(SystemCommand::CloseEntry(
                        StoreHubEntry::Recording {
                            store_id: db.store_id(),
                        },
                    ));
                }
            }
            Self::Local(app_id) => {
                ctx.command_sender()
                    .send_system(SystemCommand::CloseApp(app_id.clone()));
            }
        }
    }
}

pub fn dataset_and_its_recordings_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    kind: &EntryKind,
    mut entity_dbs: Vec<&EntityDb>,
) {
    entity_dbs.sort_by_key(|entity_db| entity_db.recording_property::<Timestamp>());

    let item = kind.item();
    let selected = ctx.selection().contains_item(&item);

    let dataset_list_item = ui
        .list_item()
        .selected(selected)
        .active(kind.is_active(ctx));
    let mut dataset_list_item_content =
        re_ui::list_item::LabelContent::new(kind.name()).with_icon(&icons::DATASET);

    let id = ui.make_persistent_id(kind);
    if !entity_dbs.is_empty() {
        dataset_list_item_content = dataset_list_item_content.with_buttons(|ui| {
            // Close-button:
            let resp = ui
                .small_icon_button(&icons::CLOSE_SMALL)
                .on_hover_text("Close all recordings in this dataset. This cannot be undone.");
            if resp.clicked() {
                kind.close(ctx, &entity_dbs);
            }
            resp
        });
    }

    let mut item_response = if !entity_dbs.is_empty() {
        dataset_list_item
            .show_hierarchical_with_children(ui, id, true, dataset_list_item_content, |ui| {
                for entity_db in &entity_dbs {
                    let include_app_id = false; // we already show it in the parent
                    entity_db_button_ui(
                        ctx,
                        ui,
                        entity_db,
                        UiLayout::SelectionPanel,
                        include_app_id,
                    );
                }
            })
            .item_response
    } else {
        dataset_list_item.show_hierarchical(ui, dataset_list_item_content)
    };

    if let EntryKind::Local(app) = &kind {
        item_response = item_response.on_hover_ui(|ui| {
            app.data_ui_recording(ctx, ui, UiLayout::Tooltip);
        });

        ctx.handle_select_hover_drag_interactions(&item_response, Item::AppId(app.clone()), false);
    }

    if item_response.clicked() {
        kind.select(ctx);
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
                entry_kind: Some(re_protos::catalog::v1alpha1::EntryKind::Dataset.into()),
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
        .scan_partition_table(ScanPartitionTableRequest {
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
