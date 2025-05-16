use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};

use egui::{Frame, Margin, RichText};

use re_log_types::EntryId;
use re_protos::manifest_registry::v1alpha1::DATASET_MANIFEST_ID_FIELD_NAME;
use re_ui::list_item::ItemActionButton;
use re_ui::{UiExt as _, icons, list_item};
use re_viewer_context::{
    AsyncRuntimeHandle, DisplayMode, Item, SystemCommand, SystemCommandSender as _, ViewerContext,
};

use crate::add_server_modal::AddServerModal;
use crate::context::Context;
use crate::entries::{Dataset, DatasetRecordings, Entries, RemoteRecordings};
use crate::tables_session_context::TablesSessionContext;

struct Server {
    origin: re_uri::Origin,
    entries: Entries,

    /// Session context wrapper which holds all the table-like entries of the server.
    tables_session_ctx: TablesSessionContext,

    runtime: AsyncRuntimeHandle,
}

impl Server {
    fn new(runtime: AsyncRuntimeHandle, egui_ctx: &egui::Context, origin: re_uri::Origin) -> Self {
        let entries = Entries::new(&runtime, egui_ctx, origin.clone());

        let tables_session_ctx = TablesSessionContext::new(&runtime, egui_ctx, origin.clone());

        Self {
            origin,
            entries,
            tables_session_ctx,
            runtime,
        }
    }

    fn refresh_entries(&mut self, runtime: &AsyncRuntimeHandle, egui_ctx: &egui::Context) {
        self.entries = Entries::new(runtime, egui_ctx, self.origin.clone());

        // Note: this also drops the DataFusionTableWidget caches
        self.tables_session_ctx.refresh(runtime, egui_ctx);
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
}
