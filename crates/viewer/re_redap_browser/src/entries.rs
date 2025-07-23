use std::task::Poll;

use ahash::HashMap;
use itertools::Itertools as _;

use re_data_ui::DataUi as _;
use re_data_ui::item_ui::entity_db_button_ui;
use re_dataframe_ui::RequestedObject;
use re_grpc_client::{ConnectionError, ConnectionRegistryHandle, StreamError};
use re_log_encoding::codec::CodecError;
use re_log_types::{ApplicationId, EntryId, natural_ordering};
use re_protos::TypeConversionError;
use re_protos::catalog::v1alpha1::ext::TableEntry;
use re_protos::catalog::v1alpha1::{EntryFilter, ext::DatasetEntry};
use re_protos::external::prost::Name as _;
use re_sorbet::SorbetError;
use re_types::archetypes::RecordingInfo;
use re_types::components::{Name, Timestamp};
use re_ui::{UiExt as _, UiLayout, icons, list_item};
use re_viewer_context::{
    AsyncRuntimeHandle, DisplayMode, Item, RecordingOrTable, SystemCommand,
    SystemCommandSender as _, ViewerContext, external::re_entity_db::EntityDb,
};

use crate::context::Context;

#[expect(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum EntryError {
    /// You usually want to use [`EntryError::tonic_status`] instead
    /// (there are multiple variants holding [`tonic::Status`]).
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
}

impl EntryError {
    fn tonic_status(&self) -> Option<&tonic::Status> {
        // Be explicit here so we don't miss any future variants that might have a `tonic::Status`.
        match self {
            Self::TonicError(status) => Some(status),
            Self::StreamError(StreamError::TonicStatus(status)) => Some(&status.0),
            #[cfg(not(target_arch = "wasm32"))]
            Self::StreamError(StreamError::Transport(_)) => None,
            Self::StreamError(
                StreamError::ConnectionError(_)
                | StreamError::Tokio(_)
                | StreamError::CodecError(_)
                | StreamError::ChunkError(_)
                | StreamError::DecodeError(_)
                | StreamError::InvalidUri(_)
                | StreamError::InvalidSorbetSchema(_)
                | StreamError::TypeConversionError(_)
                | StreamError::MissingChunkData
                | StreamError::MissingDataframeColumn(_)
                | StreamError::MissingData(_)
                | StreamError::ArrowError(_),
            )
            | Self::ConnectionError(_)
            | Self::TypeConversionError(_)
            | Self::CodecError(_)
            | Self::SorbetError(_) => None,
        }
    }

    pub fn is_missing_token(&self) -> bool {
        if let Some(status) = self.tonic_status() {
            status.code() == tonic::Code::Unauthenticated
                && status.message() == "missing credentials"
        } else {
            false
        }
    }

    pub fn is_wrong_token(&self) -> bool {
        if let Some(status) = self.tonic_status() {
            status.code() == tonic::Code::Unauthenticated
                && status.message() == "invalid credentials"
        } else {
            false
        }
    }
}

pub struct Dataset {
    pub dataset_entry: DatasetEntry,
    pub origin: re_uri::Origin,
}

impl Dataset {
    pub fn id(&self) -> EntryId {
        self.dataset_entry.details.id
    }

    pub fn name(&self) -> &str {
        self.dataset_entry.details.name.as_ref()
    }
}

pub struct Table {
    pub table_entry: TableEntry,

    pub origin: re_uri::Origin,
}

impl Table {
    pub fn id(&self) -> EntryId {
        self.table_entry.details.id
    }

    pub fn name(&self) -> &str {
        self.table_entry.details.name.as_ref()
    }
}

pub enum EntryRef<'a> {
    Dataset(&'a Dataset),
    Table(&'a Table),
}

/// All the entries of a server.
// TODO(ab): we currently load the ENTIRE list of datasets. We will need to be more granular
// about this in the future.
pub struct Entries {
    datasets: RequestedObject<Result<HashMap<EntryId, Dataset>, EntryError>>,
    tables: RequestedObject<Result<HashMap<EntryId, Table>, EntryError>>,
}

impl Entries {
    pub fn new(
        connection_registry: ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
    ) -> Self {
        let datasets_fut = fetch_dataset_entries(connection_registry.clone(), origin.clone());
        let tables_fut = fetch_table_entries(connection_registry, origin);

        Self {
            datasets: RequestedObject::new_with_repaint(runtime, egui_ctx.clone(), datasets_fut),
            tables: RequestedObject::new_with_repaint(runtime, egui_ctx.clone(), tables_fut),
        }
    }

    pub fn on_frame_start(&mut self) {
        self.datasets.on_frame_start();
        self.tables.on_frame_start();
    }

    pub fn find_dataset(&self, entry_id: EntryId) -> Option<&Dataset> {
        self.datasets.try_as_ref()?.as_ref().ok()?.get(&entry_id)
    }

    pub fn find_table(&self, entry_id: EntryId) -> Option<&Table> {
        self.tables.try_as_ref()?.as_ref().ok()?.get(&entry_id)
    }

    pub fn find_entry(&self, entry_id: EntryId) -> Option<EntryRef<'_>> {
        if let Some(dataset) = self.find_dataset(entry_id) {
            return Some(EntryRef::Dataset(dataset));
        }
        if let Some(table) = self.find_table(entry_id) {
            return Some(EntryRef::Table(table));
        }
        None
    }

    pub fn state(&self) -> Poll<Result<&HashMap<EntryId, Dataset>, &EntryError>> {
        self.datasets
            .try_as_ref()
            .map_or(Poll::Pending, |r| Poll::Ready(r.as_ref()))
    }

    /// [`list_item::ListItem`]-based UI for the datasets.
    pub fn panel_ui(
        &self,
        viewer_context: &ViewerContext<'_>,
        _ctx: &Context<'_>,
        ui: &mut egui::Ui,
        mut recordings: Option<re_entity_db::DatasetRecordings<'_>>,
    ) {
        let mut loading_things = smallvec::SmallVec::<[_; 2]>::new();
        let mut failed_things = smallvec::SmallVec::<[_; 2]>::new();
        let mut errors = smallvec::SmallVec::<[_; 2]>::new();

        match self.datasets.try_as_ref() {
            None => {
                loading_things.push("datasets");
            }

            Some(Err(err)) => {
                failed_things.push("datasets");
                errors.push(err.to_string());
            }

            Some(Ok(datasets)) => {
                for dataset in datasets.values().sorted_by_key(|dataset| dataset.name()) {
                    let recordings = recordings
                        .as_mut()
                        .and_then(|r| r.remove(&dataset.id()))
                        .unwrap_or_default();

                    dataset_and_its_recordings_ui(
                        ui,
                        viewer_context,
                        &DatasetKind::Remote {
                            origin: dataset.origin.clone(),
                            entry_id: dataset.id(),
                            name: dataset.name().to_owned(),
                        },
                        recordings,
                    );
                }
            }
        }

        match self.tables.try_as_ref() {
            None => {
                loading_things.push("tables");
            }

            Some(Err(err)) => {
                failed_things.push("tables");
                errors.push(err.to_string());
            }

            Some(Ok(tables)) => {
                for table in tables.values().sorted_by_key(|table| table.name()) {
                    table_ui(ui, viewer_context, table);
                }
            }
        }

        // TODO(#10568): these loading and error status should be displayed as a spinner/icon on the
        // parent item instead (server), but that requires improving the `list_item` API.
        if !loading_things.is_empty() {
            ui.list_item_flat_noninteractive(
                list_item::LabelContent::new(format!("Loading {}…", loading_things.join(" and ")))
                    .italics(true),
            );
        }

        if !failed_things.is_empty() {
            ui.list_item_flat_noninteractive(list_item::LabelContent::new(
                egui::RichText::new(format!("Failed to load {}", failed_things.join(" and ")))
                    .color(ui.visuals().error_fg_color),
            ))
            .on_hover_ui(|ui| {
                for error in errors.into_iter().unique() {
                    ui.label(error);
                }
            });
        }
    }
}

#[derive(Clone, Hash)]
pub enum DatasetKind {
    Remote {
        origin: re_uri::Origin,
        entry_id: EntryId,
        name: String,
    },
    Local(ApplicationId),
}

impl DatasetKind {
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
            Self::Remote { entry_id, .. } => ctx.active_redap_entry() == Some(entry_id),
            // TODO(lucasmerlin): Update this when local datasets have a view like remote datasets
            Self::Local(_) => false,
        }
    }

    fn close(&self, ctx: &ViewerContext<'_>, dbs: &Vec<&EntityDb>) {
        match self {
            Self::Remote { .. } => {
                for db in dbs {
                    ctx.command_sender()
                        .send_system(SystemCommand::CloseRecordingOrTable(
                            RecordingOrTable::Recording {
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
    kind: &DatasetKind,
    mut entity_dbs: Vec<&EntityDb>,
) {
    entity_dbs.sort_by_cached_key(|entity_db| {
        (
            entity_db
                .recording_info_property::<Name>(&RecordingInfo::descriptor_name())
                .map(|s| natural_ordering::OrderedString(s.to_string())),
            entity_db.recording_info_property::<Timestamp>(&RecordingInfo::descriptor_start_time()),
        )
    });

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
                .small_icon_button(&icons::CLOSE_SMALL, "Close all recordings in this dataset")
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

    if let DatasetKind::Local(app) = &kind {
        item_response = item_response.on_hover_ui(|ui| {
            app.data_ui_recording(ctx, ui, UiLayout::Tooltip);
        });

        ctx.handle_select_hover_drag_interactions(&item_response, Item::AppId(app.clone()), false);
    }

    if item_response.clicked() {
        kind.select(ctx);
    }
}

pub fn table_ui(ui: &mut egui::Ui, ctx: &ViewerContext<'_>, table: &Table) {
    let item = Item::RedapEntry(table.id());
    let selected = ctx.selection().contains_item(&item);
    let is_active = ctx.active_redap_entry() == Some(&table.id());

    let table_list_item = ui.list_item().selected(selected).active(is_active);
    let table_list_item_content =
        re_ui::list_item::LabelContent::new(table.name()).with_icon(&icons::TABLE);

    let item_response = table_list_item.show_hierarchical(ui, table_list_item_content);

    if item_response.clicked() {
        ctx.command_sender()
            .send_system(SystemCommand::SetSelection(item));
        ctx.command_sender()
            .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapEntry(
                table.id(),
            )));
    }
}

async fn fetch_dataset_entries(
    connection_registry: ConnectionRegistryHandle,
    origin: re_uri::Origin,
) -> Result<HashMap<EntryId, Dataset>, EntryError> {
    let mut client = connection_registry.client(origin.clone()).await?;

    let entries = client
        .find_entries(EntryFilter {
            id: None,
            name: None,
            entry_kind: Some(re_protos::catalog::v1alpha1::EntryKind::Dataset.into()),
        })
        .await?;

    let futures = entries.into_iter().map(|entry_details| {
        let mut client = client.clone();
        async move { client.read_dataset_entry(entry_details.id).await }
    });

    let mut datasets = HashMap::default();
    for dataset_entry in futures::future::join_all(futures).await {
        let dataset = Dataset {
            dataset_entry: dataset_entry?,
            origin: origin.clone(),
        };

        datasets.insert(dataset.id(), dataset);
    }

    Ok(datasets)
}

async fn fetch_table_entries(
    connection_registry: ConnectionRegistryHandle,
    origin: re_uri::Origin,
) -> Result<HashMap<EntryId, Table>, EntryError> {
    let mut client = connection_registry.client(origin.clone()).await?;

    let entries = client
        .find_entries(EntryFilter {
            id: None,
            name: None,
            entry_kind: Some(re_protos::catalog::v1alpha1::EntryKind::Table.into()),
        })
        .await?;

    let futures = entries.into_iter().map(|entry_details| {
        let mut client = client.clone();
        async move { client.read_table_entry(entry_details.id).await }
    });

    let mut tables = HashMap::default();
    for table_entry in futures::future::join_all(futures).await {
        let table_entry = table_entry?;

        // We do not want to display system tables in the UI.
        if table_entry.provider_details.type_url
            == re_protos::catalog::v1alpha1::SystemTable::type_url()
        {
            continue;
        }

        let table = Table {
            table_entry,
            origin: origin.clone(),
        };

        tables.insert(table.id(), table);
    }

    Ok(tables)
}
