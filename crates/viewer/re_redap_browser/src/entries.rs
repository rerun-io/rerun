use std::future;
use std::sync::Arc;
use std::task::Poll;

use ahash::HashMap;
use datafusion::catalog::TableProvider;
use datafusion::common::DataFusionError;
use datafusion::prelude::SessionContext;
use egui::RichText;
use futures::stream::FuturesUnordered;
use futures::{FutureExt as _, StreamExt as _, TryFutureExt as _};
use itertools::Itertools as _;
use re_data_ui::DataUi as _;
use re_data_ui::item_ui::entity_db_button_ui;
use re_dataframe_ui::RequestedObject;
use re_datafusion::{PartitionTableProvider, TableEntryTableProvider};
use re_grpc_client::{ConnectionClient, ConnectionError, ConnectionRegistryHandle, StreamError};
use re_log_encoding::codec::CodecError;
use re_log_types::{ApplicationId, EntryId, natural_ordering};
use re_protos::TypeConversionError;
use re_protos::catalog::v1alpha1::ext::{EntryDetails, TableEntry};
use re_protos::catalog::v1alpha1::{EntryFilter, EntryKind, FindEntriesRequest, ext::DatasetEntry};
use re_protos::external::prost;
use re_protos::external::prost::Name as _;
use re_sorbet::SorbetError;
use re_types::archetypes::RecordingInfo;
use re_types::components::{Name, Timestamp};
use re_ui::list_item::LabelContent;
use re_ui::{Icon, UiExt as _, UiLayout, icons, list_item};
use re_viewer_context::{
    AsyncRuntimeHandle, DisplayMode, Item, RecordingOrTable, SystemCommand,
    SystemCommandSender as _, ViewerContext, external::re_entity_db::EntityDb,
};

use crate::context::Context;

pub type EntryResult<T> = Result<T, EntryError>;

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

    #[error(transparent)]
    DataFusionError(#[from] DataFusionError),
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
            | Self::SorbetError(_)
            | Self::DataFusionError(_) => None,
        }
    }

    pub fn is_missing_token(&self) -> bool {
        if let Some(status) = self.tonic_status() {
            status.code() == tonic::Code::Unauthenticated
                && status.message() == re_auth::ERROR_MESSAGE_MISSING_CREDENTIALS
        } else {
            false
        }
    }

    pub fn is_wrong_token(&self) -> bool {
        if let Some(status) = self.tonic_status() {
            status.code() == tonic::Code::Unauthenticated
                && status.message() == re_auth::ERROR_MESSAGE_INVALID_CREDENTIALS
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

pub enum EntryInner {
    Dataset(Dataset),
    Table(Table),
}

pub struct Entry {
    details: EntryDetails,
    inner: EntryResult<EntryInner>,
}

impl Entry {
    pub fn details(&self) -> &EntryDetails {
        &self.details
    }

    pub fn id(&self) -> EntryId {
        self.details().id
    }

    pub fn name(&self) -> &str {
        &self.details().name
    }

    pub fn icon(&self) -> Icon {
        match &self.details.kind {
            EntryKind::Dataset | EntryKind::DatasetView | EntryKind::BlueprintDataset => {
                icons::DATASET
            }
            EntryKind::Table | EntryKind::TableView => icons::TABLE,
            EntryKind::Unspecified => icons::VIEW_UNKNOWN,
        }
    }

    pub fn inner(&self) -> &EntryResult<EntryInner> {
        &self.inner
    }
}

/// All the entries of a server.
// TODO(ab): we currently load the ENTIRE list of datasets. We will need to be more granular
// about this in the future.
pub struct Entries {
    entries: RequestedObject<EntryResult<HashMap<EntryId, Entry>>>,
}

impl Entries {
    pub fn new(
        connection_registry: ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
        session_context: Arc<SessionContext>,
    ) -> Self {
        let entries_fut =
            fetch_entries_and_register_tables(connection_registry, origin, session_context);

        Self {
            entries: RequestedObject::new_with_repaint(runtime, egui_ctx.clone(), entries_fut),
        }
    }

    pub fn on_frame_start(&mut self) {
        self.entries.on_frame_start();
    }

    pub fn find_entry(&self, entry_id: EntryId) -> Option<&Entry> {
        self.entries.try_as_ref()?.as_ref().ok()?.get(&entry_id)
    }

    pub fn state(&self) -> Poll<Result<&HashMap<EntryId, Entry>, &EntryError>> {
        self.entries
            .try_as_ref()
            .map_or(Poll::Pending, |r| match r {
                Ok(entries) => Poll::Ready(Ok(entries)),
                Err(err) => Poll::Ready(Err(err)),
            })
    }

    /// [`list_item::ListItem`]-based UI for the datasets.
    pub fn panel_ui(
        &self,
        viewer_context: &ViewerContext<'_>,
        _ctx: &Context<'_>,
        ui: &mut egui::Ui,
        mut recordings: Option<re_entity_db::DatasetRecordings<'_>>,
    ) {
        match self.entries.try_as_ref() {
            None => {
                // TODO(#10568): these loading and error status should be displayed as a spinner/icon on the
                // parent item instead (server), but that requires improving the `list_item` API.
                ui.list_item_flat_noninteractive(
                    list_item::LabelContent::new(format!("Loading entries…")).italics(true),
                );
            }

            Some(Err(err)) => {
                ui.list_item_flat_noninteractive(list_item::LabelContent::new(
                    egui::RichText::new("Failed to load entries")
                        .color(ui.visuals().error_fg_color),
                ))
                .on_hover_text(err.to_string());
            }

            Some(Ok(entries)) => {
                for entry in entries.values().sorted_by_key(|entry| entry.name()) {
                    match entry.inner() {
                        Ok(EntryInner::Dataset(dataset)) => {
                            let recordings = recordings
                                .as_mut()
                                .and_then(|r| r.remove(&dataset.id()))
                                .unwrap_or_default();

                            dataset_list_item_and_its_recordings_ui(
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
                        Ok(EntryInner::Table(_)) | Err(_) => {
                            entry_list_item_ui(ui, viewer_context, entry)
                        }
                    }
                }
            }
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
                                store_id: db.store_id().clone(),
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

pub fn dataset_list_item_and_its_recordings_ui(
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

pub fn entry_list_item_ui(ui: &mut egui::Ui, ctx: &ViewerContext<'_>, entry: &Entry) {
    let item = Item::RedapEntry(entry.id());
    let selected = ctx.selection().contains_item(&item);
    let is_active = ctx.active_redap_entry() == Some(&entry.id());
    let is_error = entry.inner().is_err();

    let mut text = RichText::new(entry.name());
    if is_error {
        text = text.color(ui.visuals().error_fg_color);
    }
    let table_list_item = ui.list_item().selected(selected).active(is_active);
    let icon = entry.icon();
    let table_list_item_content = LabelContent::new(text).with_icon(&icon);

    let item_response = table_list_item.show_hierarchical(ui, table_list_item_content);

    if item_response.clicked() {
        ctx.command_sender()
            .send_system(SystemCommand::SetSelection(item));
        ctx.command_sender()
            .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapEntry(
                entry.id(),
            )));
    }

    if let Err(err) = entry.inner() {
        item_response.on_hover_text(err.to_string());
    }
}

async fn fetch_entries_and_register_tables(
    connection_registry: ConnectionRegistryHandle,
    origin: re_uri::Origin,
    session_ctx: Arc<SessionContext>,
) -> EntryResult<HashMap<EntryId, Entry>> {
    let mut client = connection_registry.client(origin.clone()).await?;

    let entries = client
        .inner()
        .find_entries(FindEntriesRequest {
            filter: Some(EntryFilter {
                id: None,
                name: None,
                entry_kind: None,
            }),
        })
        .await?
        .into_inner()
        .entries
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<EntryDetails>, _>>()?;

    let origin_ref = &origin;
    let futures_iter = entries
        .into_iter()
        .filter_map(move |e| fetch_entry_details(client.clone(), origin_ref, e));

    let mut entries = HashMap::default();

    let mut futures_unordered = FuturesUnordered::from_iter(futures_iter);
    while let Some((id, (details, result))) = futures_unordered.next().await {
        let inner_result = result.map(|(inner, provider)| {
            session_ctx.register_table(&details.name, provider).ok();
            inner
        });

        let is_system_table = match &inner_result {
            Ok(EntryInner::Dataset(_)) => false,
            Ok(EntryInner::Table(table)) => {
                table.table_entry.provider_details.type_url
                    == re_protos::catalog::v1alpha1::SystemTable::type_url()
            }
            Err(_) => false,
        };
        if !is_system_table {
            let entry = Entry {
                details,
                inner: inner_result,
            };
            entries.insert(id, entry);
        }
    }

    Ok(entries)
}

/// Returns None if the entry should not be presented in the UI.
fn fetch_entry_details(
    client: ConnectionClient,
    origin: &re_uri::Origin,
    entry: EntryDetails,
) -> Option<
    impl Future<
        Output = (
            EntryId,
            (
                EntryDetails,
                EntryResult<(EntryInner, Arc<dyn TableProvider>)>,
            ),
        ),
    >,
> {
    let id = entry.id;
    let entry_clone = entry.clone();
    #[expect(clippy::match_same_arms)]
    match entry.kind {
        // TODO(rerun-io/dataplatform#857): these are often empty datasets, and thus fail. For
        // some reason, this failure is silent but blocks other tables from being registered.
        // Since we don't need these tables yet, we just skip them for now.
        EntryKind::BlueprintDataset => None,
        EntryKind::Dataset => Some(
            fetch_dataset_details(client, entry, origin)
                .map_ok(|(dataset, table_provider)| (EntryInner::Dataset(dataset), table_provider))
                .map(move |res| (id, (entry_clone, res)))
                .boxed(),
        ),
        EntryKind::Table => Some(
            fetch_table_details(client, entry, origin)
                .map_ok(|(table, table_provider)| (EntryInner::Table(table), table_provider))
                .map(move |res| (id, (entry_clone, res)))
                .boxed(),
        ),

        // TODO(ab): these do not exist yet
        EntryKind::DatasetView | EntryKind::TableView => None,

        EntryKind::Unspecified => Some(
            future::ready((
                id,
                (
                    entry_clone,
                    Err(
                        TypeConversionError::from(prost::UnknownEnumValue(entry.kind as i32))
                            .into(),
                    ),
                ),
            ))
            .boxed(),
        ),
    }
}

async fn fetch_dataset_details(
    mut client: ConnectionClient,
    entry: EntryDetails,
    origin: &re_uri::Origin,
) -> EntryResult<(Dataset, Arc<dyn TableProvider>)> {
    let result = client
        .read_dataset_entry(entry.id)
        .await
        .map(|dataset_entry| Dataset {
            dataset_entry,
            origin: origin.clone(),
        })?;

    let table_provider = PartitionTableProvider::new(client, entry.id)
        .into_provider()
        .await?;

    Ok((result, table_provider))
}

async fn fetch_table_details(
    mut client: ConnectionClient,
    entry: EntryDetails,
    origin: &re_uri::Origin,
) -> EntryResult<(Table, Arc<dyn TableProvider>)> {
    let result = client
        .read_table_entry(entry.id)
        .await
        .map(|table_entry| Table {
            table_entry,
            origin: origin.clone(),
        })?;

    let table_provider = TableEntryTableProvider::new(client, entry.id)
        .into_provider()
        .await?;

    Ok((result, table_provider))
}
