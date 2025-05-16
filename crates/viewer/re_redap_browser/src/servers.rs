#![allow(clippy::unwrap_used)] // TODO: do not commit

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use egui::{Frame, Margin, RichText};

use re_grpc_client::redap::RedapClient;
use re_log_encoding::codec::wire::encoder::Encode as _;
use re_log_types::external::re_tuid::Tuid;
use re_log_types::{EntryId, StoreId};
use re_protos::catalog::v1alpha1::{CreateDatasetEntryRequest, DeleteEntryRequest};
use re_protos::manifest_registry::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;
use re_ui::list_item::ItemActionButton;
use re_ui::{UiExt as _, icons, list_item};
use re_viewer_context::external::re_chunk_store::Chunk;
use re_viewer_context::external::re_entity_db::EntityDb;
use re_viewer_context::{
    AsyncRuntimeHandle, DisplayMode, Item, SystemCommand, SystemCommandSender as _, ViewerContext,
};

use crate::add_server_modal::AddServerModal;
use crate::context::Context;
use crate::entries::{Dataset, DatasetRecordings, RemoteRecordings, ServerEntry};
use crate::tables_session_context::TablesSessionContext;

struct Server {
    origin: re_uri::Origin,
    entries: ServerEntry,

    /// Session context wrapper which holds all the table-like entries of the server.
    tables_session_ctx: TablesSessionContext,

    runtime: AsyncRuntimeHandle,
}

impl Server {
    fn new(runtime: AsyncRuntimeHandle, egui_ctx: &egui::Context, origin: re_uri::Origin) -> Self {
        let entries = ServerEntry::new(&runtime, egui_ctx, origin.clone());

        let tables_session_ctx = TablesSessionContext::new(&runtime, egui_ctx, origin.clone());

        Self {
            origin,
            entries,
            tables_session_ctx,
            runtime,
        }
    }

    fn refresh_entries(&mut self, runtime: &AsyncRuntimeHandle, egui_ctx: &egui::Context) {
        self.entries = ServerEntry::new(runtime, egui_ctx, self.origin.clone());

        // Note: this also drops the DataFusionTableWidget caches
        self.tables_session_ctx = TablesSessionContext::new(runtime, egui_ctx, self.origin.clone());
    }

    fn on_frame_start(&mut self) {
        self.entries.on_frame_start();
        self.tables_session_ctx.on_frame_start();
    }

    fn find_dataset(&self, entry_id: EntryId) -> Option<&Dataset> {
        self.entries.find_dataset(entry_id)
    }

    /// Central panel UI for when a server is selected.
    fn server_ui(&self, ctx: &Context<'_>, ui: &mut egui::Ui) {
        Frame::new()
            .inner_margin(Margin {
                top: 16,
                bottom: 12,
                left: 16,
                right: 16,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Catalog").strong());
                    if ui.small_icon_button(&icons::RESET).clicked() {
                        ctx.command_sender
                            .send(Command::RefreshCollection(self.origin.clone()))
                            .ok();
                    }
                });

                ui.add_space(12.0);

                ui.list_item_scope(
                    egui::Id::new(&self.origin).with("catalog server ui"),
                    |ui| {
                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("Address").value_fn(|ui, _| {
                                ui.strong(self.origin.to_string());
                            }),
                        );

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("Datasets").value_fn(|ui, _| {
                                match self.entries.dataset_count() {
                                    None => ui.label("loading…"),
                                    Some(Ok(count)) => ui.strong(format!("{count}")),
                                    Some(Err(err)) => ui
                                        .error_label("could not load entries")
                                        .on_hover_text(err.to_string()),
                                };
                            }),
                        );

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("Tables").value_fn(|ui, _| {
                                ui.strong(format!("{}", self.entries.table_count()));
                            }),
                        );
                    },
                );
            });
    }

    fn dataset_entry_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ctx: &Context<'_>,
        ui: &mut egui::Ui,
        dataset: &Dataset,
    ) {
        re_dataframe_ui::DataFusionTableWidget::new(
            self.tables_session_ctx.ctx.clone(),
            dataset.name(),
        )
        .title(dataset.name())
        .title_button(ItemActionButton::new(&re_ui::icons::RESET, || {
            ctx.command_sender
                .send(Command::RefreshCollection(self.origin.clone()))
                .ok();
        }))
        .column_renamer(|desc| {
            //TODO(ab): with this strategy, we do not display relevant entity path if any.
            let name = desc.display_name();

            name.strip_prefix("rerun_")
                .unwrap_or(name.as_ref())
                .replace('_', " ")
        })
        .generate_partition_links(
            "recording link",
            DATASET_MANIFEST_ID_FIELD_NAME,
            self.origin.clone(),
            dataset.id(),
        )
        .show(viewer_ctx, &self.runtime, ui);
    }

    fn panel_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ctx: &Context<'_>,
        ui: &mut egui::Ui,
        recordings: Option<DatasetRecordings<'_>>,
    ) {
        let item = Item::RedapServer(self.origin.clone());
        let is_selected = viewer_ctx.selection().contains_item(&item);

        let content =
            list_item::LabelContent::header(self.origin.host.to_string()).with_buttons(|ui| {
                let response = ui
                    .small_icon_button(&re_ui::icons::REMOVE)
                    .on_hover_text("Remove server");

                if response.clicked() {
                    ctx.command_sender
                        .send(Command::RemoveServer(self.origin.clone()))
                        .ok();
                }

                response
            });

        let item_response = ui
            .list_item()
            .header()
            .selected(is_selected)
            .show_hierarchical_with_children(
                ui,
                egui::Id::new(&self.origin).with("server_item"),
                true,
                content,
                |ui| {
                    self.entries.panel_ui(viewer_ctx, ctx, ui, recordings);
                },
            )
            .item_response
            .on_hover_text(self.origin.to_string());

        viewer_ctx.handle_select_hover_drag_interactions(&item_response, item, false);

        if item_response.clicked() {
            viewer_ctx
                .command_sender()
                .send_system(SystemCommand::ChangeDisplayMode(DisplayMode::RedapServer(
                    self.origin.clone(),
                )));
        }
    }

    fn upload_to_dataset(
        &self,
        dataset_name: String,
        entity_dbs: &[&EntityDb],
        command_sender: Sender<Command>,
        create_new: bool,
    ) {
        let chunks: Vec<(StoreId, Vec<Arc<Chunk>>)> = entity_dbs
            .iter()
            .map(|entity_db| {
                (
                    entity_db.store_id(),
                    entity_db
                        .storage_engine()
                        .store()
                        .iter_chunks()
                        .cloned()
                        .collect(),
                )
            })
            .collect();

        let dataset_id = if create_new {
            None
        } else {
            let Some(dataset) = self.entries.find_dataset_by_name(&dataset_name) else {
                re_log::error!("Dataset not found: {dataset_name}");
                return;
            };
            Some(dataset.id())
        };

        let origin = self.origin.clone();
        let num_dbs = entity_dbs.len();

        self.runtime.spawn_future(async move {
            let mut client = match re_grpc_client::redap::client(origin.clone()).await {
                Ok(client) => client,
                Err(err) => {
                    re_log::error!("Failed to connect to {origin:?}: {err}");
                    return;
                }
            };

            let dataset_id = if let Some(dataset_id) = dataset_id {
                dataset_id
            } else {
                let result = create_new_dataset(&mut client, dataset_name).await;
                match result {
                    Ok(id) => id,
                    Err(err) => {
                        re_log::error!("Failed to create new dataset: {err}");
                        return;
                    }
                }
            };

            let result = write_chunks_to_dataset(&mut client, &dataset_id, &chunks).await;
            if let Err(err) = result {
                re_log::error!("Failed to upload dataset: {err}");
            } else {
                re_log::info!("Successfully uploaded {} recordings to {origin:?}", num_dbs);

                // Kick off a refresh of the server.
                command_sender.send(Command::RefreshCollection(origin)).ok();
            }
        });
    }

    fn delete_entry(&self, entry: EntryId, command_sender: Sender<Command>) {
        let origin = self.origin.clone();

        self.runtime.spawn_future(async move {
            let mut client = match re_grpc_client::redap::client(origin.clone()).await {
                Ok(client) => client,
                Err(err) => {
                    re_log::error!("Failed to connect to {origin:?}: {err}");
                    return;
                }
            };

            let result = delete_entry(&mut client, entry).await;
            if let Err(err) = result {
                re_log::error!("Failed to delete entry: {err}");
            } else {
                re_log::info!("Successfully deleted entry: {origin:?}");

                // Kick off a refresh of the server.
                command_sender.send(Command::RefreshCollection(origin)).ok();
            }
        });
    }
}

/// All servers known to the viewer, and their catalog data.
pub struct RedapServers {
    servers: BTreeMap<re_uri::Origin, Server>,

    /// When deserializing we can't construct the [`Server`]s right away
    /// so they get queued here.
    pending_servers: Vec<re_uri::Origin>,

    // message queue for commands
    command_sender: Sender<Command>,
    command_receiver: Receiver<Command>,

    add_server_modal_ui: AddServerModal,
}

impl serde::Serialize for RedapServers {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.servers
            .keys()
            .collect::<Vec<_>>()
            .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for RedapServers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let origins = Vec::<re_uri::Origin>::deserialize(deserializer)?;

        let mut servers = Self::default();

        // We cannot create `Server` right away, because we need an async handle and an
        // `egui::Context` for that, so we just queue commands to be processed early next frame.
        for origin in origins {
            servers.pending_servers.push(origin);
        }

        Ok(servers)
    }
}

impl Default for RedapServers {
    fn default() -> Self {
        let (command_sender, command_receiver) = std::sync::mpsc::channel();

        Self {
            servers: Default::default(),
            pending_servers: Default::default(),
            command_sender,
            command_receiver,
            add_server_modal_ui: Default::default(),
        }
    }
}

pub enum Command {
    OpenAddServerModal,
    AddServer(re_uri::Origin),
    RemoveServer(re_uri::Origin),
    RefreshCollection(re_uri::Origin),
}

impl RedapServers {
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty() && self.pending_servers.is_empty()
    }

    /// Whether we already know about a given server (or have it queued to be added).
    pub fn has_server(&self, origin: &re_uri::Origin) -> bool {
        self.servers.contains_key(origin) || self.pending_servers.contains(origin)
    }

    /// Add a server to the hub.
    pub fn add_server(&self, origin: re_uri::Origin) {
        self.command_sender.send(Command::AddServer(origin)).ok();
    }

    pub fn that_list(&self) -> impl Iterator<Item = (re_uri::Origin, Vec<String>)> {
        self.servers.iter().map(|(origin, server)| {
            (
                origin.clone(),
                server.entries.dataset_names().map_or(Vec::new(), |names| {
                    names.map(|name| name.to_owned()).collect::<Vec<String>>()
                }),
            )
        })
    }

    /// Per-frame housekeeping.
    ///
    /// - Process commands from the queue.
    /// - Update all servers.
    pub fn on_frame_start(&mut self, runtime: &AsyncRuntimeHandle, egui_ctx: &egui::Context) {
        self.pending_servers.drain(..).for_each(|origin| {
            self.command_sender.send(Command::AddServer(origin)).ok();
        });
        while let Ok(command) = self.command_receiver.try_recv() {
            self.handle_command(runtime, egui_ctx, command);
        }

        for server in self.servers.values_mut() {
            server.on_frame_start();
        }
    }

    fn handle_command(
        &mut self,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        command: Command,
    ) {
        match command {
            Command::OpenAddServerModal => {
                self.add_server_modal_ui.open();
            }

            Command::AddServer(origin) => {
                if !self.servers.contains_key(&origin) {
                    self.servers.insert(
                        origin.clone(),
                        Server::new(runtime.clone(), egui_ctx, origin.clone()),
                    );
                } else {
                    // Since we persist the server list on disk this happens quite often.
                    // E.g. run `pixi run rerun "rerun+http://localhost"` more than once.
                    re_log::debug!(
                        "Tried to add pre-existing server at {:?}",
                        origin.to_string()
                    );
                }
            }

            Command::RemoveServer(origin) => {
                self.servers.remove(&origin);
            }

            Command::RefreshCollection(origin) => {
                self.servers.entry(origin).and_modify(|server| {
                    server.refresh_entries(runtime, egui_ctx);
                });
            }
        }
    }

    pub fn server_central_panel_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        origin: &re_uri::Origin,
    ) {
        if let Some(server) = self.servers.get(origin) {
            self.with_ctx(|ctx| {
                server.server_ui(ctx, ui);
            });
        } else {
            viewer_ctx
                .command_sender()
                .send_system(SystemCommand::ChangeDisplayMode(
                    DisplayMode::LocalRecordings,
                ));
        }
    }

    pub fn server_list_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        mut remote_recordings: RemoteRecordings<'_>,
    ) {
        self.with_ctx(|ctx| {
            for server in self.servers.values() {
                let recordings = remote_recordings.remove(&server.origin);
                server.panel_ui(viewer_ctx, ctx, ui, recordings);
            }
        });
    }

    pub fn open_add_server_modal(&self) {
        self.command_sender.send(Command::OpenAddServerModal).ok();
    }

    pub fn entry_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        active_entry: EntryId,
    ) {
        for server in self.servers.values() {
            if let Some(dataset) = server.find_dataset(active_entry) {
                self.with_ctx(|ctx| {
                    server.dataset_entry_ui(viewer_ctx, ctx, ui, dataset);
                });

                return;
            }
        }
    }

    pub fn modals_ui(&mut self, ui: &egui::Ui) {
        //TODO(ab): borrow checker doesn't let me use `with_ctx()` here, I should find a better way
        let ctx = Context {
            command_sender: &self.command_sender,
        };

        self.add_server_modal_ui.ui(&ctx, ui);
    }

    #[inline]
    fn with_ctx<R>(&self, func: impl FnOnce(&Context<'_>) -> R) -> R {
        let ctx = Context {
            command_sender: &self.command_sender,
        };

        func(&ctx)
    }

    pub fn upload_to_dataset(
        &self,
        entity_dbs: &[&EntityDb],
        target_server: re_uri::Origin,
        dataset_name: String,
        create_new: bool,
    ) {
        let Some(server) = self.servers.get(&target_server) else {
            re_log::error!("Not connected to server at {target_server:?}");
            return;
        };

        if create_new {}

        server.upload_to_dataset(
            dataset_name,
            entity_dbs,
            self.command_sender.clone(),
            create_new,
        );
    }

    pub fn delete_entry(&self, target_server: re_uri::Origin, entry: EntryId) {
        let Some(server) = self.servers.get(&target_server) else {
            re_log::error!("Not connected to server at {target_server:?}");
            return;
        };

        server.delete_entry(entry, self.command_sender.clone());
    }
}

async fn write_chunks_to_dataset(
    client: &mut RedapClient,
    dataset_id: &EntryId,
    chunks_per_partition: &[(StoreId, Vec<Arc<Chunk>>)],
) -> Result<(), crate::entries::EntryError> {
    re_log::debug!(
        "Writing {} recordings to dataset",
        chunks_per_partition.len()
    );

    // TODO: can't bundle those chunks into a single request.
    for (_store_id, chunks) in chunks_per_partition {
        // TODO: it's a bit unclear what the semantics of duplicating the partition are.
        // Viewer gets very confused if it sees the same partition ID twice.
        let partition_id_new = StoreId::random(re_log_types::StoreKind::Recording);
        let partition_id = &partition_id_new;

        let chunk_requests = chunks
            .iter()
            .map(move |chunk| {
                // TODO: Have to patch partition id in the metadata.
                let sorbet_batch = chunk.to_chunk_batch().unwrap();
                let mut metadata = sorbet_batch.schema().metadata().clone();
                metadata.insert(
                    "rerun.partition_id".to_owned(),
                    partition_id.as_str().to_owned(),
                );
                let schema = (*sorbet_batch.schema()).clone().with_metadata(metadata);
                let patched_batch =
                    <datafusion::arrow::array::RecordBatch as Clone>::clone(&sorbet_batch)
                        .with_schema(Arc::new(schema))
                        .unwrap();

                let chunk: re_protos::common::v1alpha1::RerunChunk =
                    patched_batch.encode().unwrap();

                re_protos::manifest_registry::v1alpha1::WriteChunksRequest { chunk: Some(chunk) }
            })
            .collect::<Vec<_>>(); // TODO: no.

        re_log::debug!("Writing {} chunks to dataset", chunk_requests.len());

        let reqs = ::futures::stream::iter(chunk_requests);
        let mut request = tonic::Request::new(reqs);
        request.metadata_mut().insert(
            "x-rerun-dataset-id",
            dataset_id.to_string().parse().unwrap(),
        );

        client.write_chunks(request).await?;
    }

    Ok(())
}

async fn create_new_dataset(
    client: &mut RedapClient,
    dataset_name: String,
) -> Result<EntryId, crate::entries::EntryError> {
    let response = client
        .create_dataset_entry(CreateDatasetEntryRequest {
            name: Some(dataset_name),
        })
        .await?;

    let response = response.into_inner();

    let id = response
        .dataset
        .ok_or(crate::entries::EntryError::FieldNotSet("dataset"))?
        .dataset_handle
        .ok_or(crate::entries::EntryError::FieldNotSet("dataset_handle"))?
        .entry_id
        .ok_or(crate::entries::EntryError::FieldNotSet("entry_id"))?
        .id
        .ok_or(crate::entries::EntryError::FieldNotSet("id"))?;
    let time_ns = id
        .time_ns
        .ok_or(crate::entries::EntryError::FieldNotSet("time_ns"))?;
    let inc = id
        .inc
        .ok_or(crate::entries::EntryError::FieldNotSet("inc"))?;

    Ok(EntryId {
        id: Tuid::from_nanos_and_inc(time_ns, inc),
    })
}

async fn delete_entry(
    client: &mut RedapClient,
    entry: EntryId,
) -> Result<(), crate::entries::EntryError> {
    client
        .delete_entry(DeleteEntryRequest {
            id: Some(entry.into()),
        })
        .await?;

    Ok(())
}
