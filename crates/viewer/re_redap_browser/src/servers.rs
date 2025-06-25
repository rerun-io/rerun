use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};

use datafusion::prelude::{col, lit};
use egui::Widget as _;

use re_dataframe_ui::{ColumnBlueprint, default_display_name_for_column};
use re_grpc_client::ConnectionRegistryHandle;
use re_log_types::{EntityPathPart, EntryId};
use re_protos::catalog::v1alpha1::EntryKind;
use re_protos::manifest_registry::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;
use re_sorbet::{BatchType, ColumnDescriptorRef};
use re_ui::list_item::{ItemActionButton, ItemButton as _, ItemMenuButton};
use re_ui::{UiExt as _, icons, list_item};
use re_viewer_context::{
    AsyncRuntimeHandle, DisplayMode, GlobalContext, Item, SystemCommand, SystemCommandSender as _,
    ViewerContext,
};

use crate::context::Context;
use crate::entries::{Dataset, Entries};
use crate::server_modal::{ServerModal, ServerModalMode};
use crate::tables_session_context::TablesSessionContext;

struct Server {
    origin: re_uri::Origin,
    entries: Entries,

    /// Session context wrapper which holds all the table-like entries of the server.
    tables_session_ctx: TablesSessionContext,

    connection_registry: re_grpc_client::ConnectionRegistryHandle,
    runtime: AsyncRuntimeHandle,
}

impl Server {
    fn new(
        connection_registry: re_grpc_client::ConnectionRegistryHandle,
        runtime: AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        origin: re_uri::Origin,
    ) -> Self {
        let entries = Entries::new(
            connection_registry.clone(),
            &runtime,
            egui_ctx,
            origin.clone(),
        );

        let tables_session_ctx = TablesSessionContext::new(
            connection_registry.clone(),
            &runtime,
            egui_ctx,
            origin.clone(),
        );

        Self {
            origin,
            entries,
            tables_session_ctx,
            connection_registry,
            runtime,
        }
    }

    fn refresh_entries(&mut self, runtime: &AsyncRuntimeHandle, egui_ctx: &egui::Context) {
        self.entries = Entries::new(
            self.connection_registry.clone(),
            runtime,
            egui_ctx,
            self.origin.clone(),
        );

        // Note: this also drops the DataFusionTableWidget caches
        self.tables_session_ctx = TablesSessionContext::new(
            self.connection_registry.clone(),
            runtime,
            egui_ctx,
            self.origin.clone(),
        );
    }

    fn on_frame_start(&mut self) {
        self.entries.on_frame_start();
        self.tables_session_ctx.on_frame_start();
    }

    fn find_dataset(&self, entry_id: EntryId) -> Option<&Dataset> {
        self.entries.find_dataset(entry_id)
    }

    /// Central panel UI for when a server is selected.
    fn server_ui(&self, viewer_ctx: &ViewerContext<'_>, ctx: &Context<'_>, ui: &mut egui::Ui) {
        const ENTRY_LINK_COLUMN_NAME: &str = "link";

        re_dataframe_ui::DataFusionTableWidget::new(
            self.tables_session_ctx.ctx.clone(),
            "__entries",
        )
        .title(self.origin.host.to_string())
        .title_button(ItemActionButton::new(
            &re_ui::icons::RESET,
            "Refresh collection",
            || {
                ctx.command_sender
                    .send(Command::RefreshCollection(self.origin.clone()))
                    .ok();
            },
        ))
        .column_blueprint(|desc| {
            let mut blueprint = ColumnBlueprint::default();

            if let ColumnDescriptorRef::Component(component) = desc {
                if component.component == "entry_kind" {
                    blueprint = blueprint.variant_ui(re_component_ui::REDAP_ENTRY_KIND_VARIANT);
                }
            }

            let column_sort_key = match desc.display_name().as_str() {
                "name" => 0,
                ENTRY_LINK_COLUMN_NAME => 1,
                _ => 2,
            };

            blueprint = blueprint.sort_key(column_sort_key);

            if desc.display_name().as_str() == ENTRY_LINK_COLUMN_NAME {
                blueprint = blueprint.variant_ui(re_component_ui::REDAP_URI_BUTTON_VARIANT);
            }

            blueprint
        })
        .generate_entry_links(ENTRY_LINK_COLUMN_NAME, "id", self.origin.clone())
        .filter(
            col("entry_kind")
                .in_list(
                    vec![lit(EntryKind::Table as i32), lit(EntryKind::Dataset as i32)],
                    false,
                )
                .and(col("name").not_eq(lit("__entries"))),
        )
        .show(viewer_ctx, &self.runtime, ui);
    }

    fn dataset_entry_ui(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        ctx: &Context<'_>,
        ui: &mut egui::Ui,
        dataset: &Dataset,
    ) {
        const RECORDING_LINK_COLUMN_NAME: &str = "recording link";

        re_dataframe_ui::DataFusionTableWidget::new(
            self.tables_session_ctx.ctx.clone(),
            dataset.name(),
        )
        .title(dataset.name())
        .title_button(ItemActionButton::new(
            &re_ui::icons::RESET,
            "Refresh collection",
            || {
                ctx.command_sender
                    .send(Command::RefreshCollection(self.origin.clone()))
                    .ok();
            },
        ))
        .column_blueprint(|desc| {
            let mut name = default_display_name_for_column(desc);

            // strip prefix and remove underscores, _only_ for the base columns (aka not the
            // properties)
            name = name
                .strip_prefix("rerun_")
                .map(|name| name.replace('_', " "))
                .unwrap_or(name);

            let default_visible = if desc.entity_path().is_some_and(|entity_path| {
                entity_path.starts_with(&std::iter::once(EntityPathPart::properties()).collect())
            }) {
                // Property column, just hide indicator components
                //TODO(#8129): remove this when we no longer have indicator components
                !desc
                    .column_name(BatchType::Dataframe)
                    .ends_with("Indicator")
            } else {
                matches!(
                    desc.display_name().as_str(),
                    RECORDING_LINK_COLUMN_NAME | DATASET_MANIFEST_ID_FIELD_NAME
                )
            };

            let column_sort_key = match desc.display_name().as_str() {
                DATASET_MANIFEST_ID_FIELD_NAME => 0,
                RECORDING_LINK_COLUMN_NAME => 1,
                _ => 2,
            };

            let mut blueprint = ColumnBlueprint::default()
                .display_name(name)
                .default_visibility(default_visible)
                .sort_key(column_sort_key);

            if desc.display_name().as_str() == RECORDING_LINK_COLUMN_NAME {
                blueprint = blueprint.variant_ui(re_component_ui::REDAP_URI_BUTTON_VARIANT);
            }

            blueprint
        })
        .generate_partition_links(
            RECORDING_LINK_COLUMN_NAME,
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
        recordings: Option<re_entity_db::DatasetRecordings<'_>>,
    ) {
        let item = Item::RedapServer(self.origin.clone());
        let is_selected = viewer_ctx.selection().contains_item(&item);
        let is_active = matches!(
            viewer_ctx.display_mode(),
            DisplayMode::RedapServer(origin)
            if origin == &self.origin
        );

        let content = list_item::LabelContent::header(self.origin.host.to_string())
            .always_show_buttons(true)
            .with_buttons(|ui| {
                Box::new(ItemMenuButton::new(&icons::MORE, "Actions", |ui| {
                    if icons::RESET
                        .as_button_with_label(ui.tokens(), "Refresh")
                        .ui(ui)
                        .clicked()
                    {
                        ctx.command_sender
                            .send(Command::RefreshCollection(self.origin.clone()))
                            .ok();
                    }
                    if icons::SETTINGS
                        .as_button_with_label(ui.tokens(), "Edit")
                        .ui(ui)
                        .clicked()
                    {
                        ctx.command_sender
                            .send(Command::OpenEditServerModal(self.origin.clone()))
                            .ok();
                    }
                    if icons::TRASH
                        .as_button_with_label(ui.tokens(), "Remove")
                        .ui(ui)
                        .clicked()
                    {
                        ctx.command_sender
                            .send(Command::RemoveServer(self.origin.clone()))
                            .ok();
                    }
                }))
                .ui(ui)
            });

        let item_response = ui
            .list_item()
            .header()
            .selected(is_selected)
            .active(is_active)
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

    server_modal_ui: ServerModal,
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
            server_modal_ui: Default::default(),
        }
    }
}

pub enum Command {
    OpenAddServerModal,
    OpenEditServerModal(re_uri::Origin),
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

    /// Per-frame housekeeping.
    ///
    /// - Process commands from the queue.
    /// - Update all servers.
    pub fn on_frame_start(
        &mut self,
        connection_registry: &ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
    ) {
        self.pending_servers.drain(..).for_each(|origin| {
            self.command_sender.send(Command::AddServer(origin)).ok();
        });
        while let Ok(command) = self.command_receiver.try_recv() {
            self.handle_command(connection_registry, runtime, egui_ctx, command);
        }

        for server in self.servers.values_mut() {
            server.on_frame_start();
        }
    }

    fn handle_command(
        &mut self,
        connection_registry: &re_grpc_client::ConnectionRegistryHandle,
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        command: Command,
    ) {
        match command {
            Command::OpenAddServerModal => {
                self.server_modal_ui
                    .open(ServerModalMode::Add, connection_registry);
            }

            Command::OpenEditServerModal(origin) => {
                self.server_modal_ui
                    .open(ServerModalMode::Edit(origin), connection_registry);
            }

            Command::AddServer(origin) => {
                if !self.servers.contains_key(&origin) {
                    self.servers.insert(
                        origin.clone(),
                        Server::new(
                            connection_registry.clone(),
                            runtime.clone(),
                            egui_ctx,
                            origin.clone(),
                        ),
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
                server.server_ui(viewer_ctx, ctx, ui);
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
        mut remote_recordings: re_entity_db::RemoteRecordings<'_>,
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

    pub fn modals_ui(
        &mut self,
        global_ctx: &GlobalContext<'_>,
        connection_registry: &ConnectionRegistryHandle,
        ui: &egui::Ui,
    ) {
        //TODO(ab): borrow checker doesn't let me use `with_ctx()` here, I should find a better way
        let ctx = Context {
            command_sender: &self.command_sender,
        };

        self.server_modal_ui
            .ui(global_ctx, &ctx, connection_registry, ui);
    }

    #[inline]
    fn with_ctx<R>(&self, func: impl FnOnce(&Context<'_>) -> R) -> R {
        let ctx = Context {
            command_sender: &self.command_sender,
        };

        func(&ctx)
    }
}
