use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender};

use re_ui::{list_item, UiExt};
use re_viewer_context::{AsyncRuntimeHandle, ViewerContext};

use crate::add_server_modal::AddServerModal;
use crate::collections::{Collection, CollectionId, Collections};
use crate::context::Context;

struct Server {
    origin: re_uri::Origin,

    collections: Collections,
}

impl Server {
    fn new(runtime: &AsyncRuntimeHandle, egui_ctx: &egui::Context, origin: re_uri::Origin) -> Self {
        //let default_catalog = FetchCollectionTask::new(runtime, origin.clone());

        let mut collections = Collections::default();

        //TODO(ab): For now, we just auto-download the default collection
        collections.add(runtime, egui_ctx, origin.clone());

        Self {
            origin,
            collections,
        }
    }

    fn on_frame_start(&mut self) {
        self.collections.on_frame_start();
    }

    fn find_collection(&self, collection_id: CollectionId) -> Option<&Collection> {
        self.collections.find(collection_id)
    }

    fn panel_ui(&self, ctx: &Context<'_>, ui: &mut egui::Ui) {
        let content = list_item::LabelContent::new(self.origin.to_string())
            .with_buttons(|ui| {
                let response = ui
                    .small_icon_button(&re_ui::icons::REMOVE)
                    .on_hover_text("Remove server");

                if response.clicked() {
                    let _ = ctx
                        .command_sender
                        .send(Command::RemoveServer(self.origin.clone()));
                }

                response
            })
            .always_show_buttons(true);

        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                egui::Id::new(&self.origin).with("server_item"),
                true,
                content,
                |ui| {
                    self.collections.panel_ui(ctx, ui);
                },
            );
    }
}

/// All servers known to the viewer, and their catalog data.
pub struct RedapServers {
    servers: BTreeMap<re_uri::Origin, Server>,

    selected_collection: Option<CollectionId>,

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

        let servers = Self::default();

        // We cannot create `Server` right away, because we need an async handle and an
        // `egui::Context` for that, so we just queue commands to be processed early next frame.
        for origin in origins {
            let _ = servers.command_sender.send(Command::AddServer(origin));
        }

        Ok(servers)
    }
}

impl Default for RedapServers {
    fn default() -> Self {
        let (command_sender, command_receiver) = std::sync::mpsc::channel();

        Self {
            servers: Default::default(),
            selected_collection: None,
            command_sender,
            command_receiver,
            add_server_modal_ui: Default::default(),
        }
    }
}

pub enum Command {
    SelectCollection(CollectionId),
    DeselectCollection,
    AddServer(re_uri::Origin),
    RemoveServer(re_uri::Origin),
}

impl RedapServers {
    /// Add a server to the hub.
    pub fn add_server(&self, origin: re_uri::Origin) {
        let _ = self.command_sender.send(Command::AddServer(origin));
    }

    /// Per-frame housekeeping.
    ///
    /// - Process commands from the queue.
    /// - Update all servers.
    pub fn on_frame_start(&mut self, runtime: &AsyncRuntimeHandle, egui_ctx: &egui::Context) {
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
            Command::SelectCollection(collection_handle) => {
                self.selected_collection = Some(collection_handle);
            }

            Command::DeselectCollection => self.selected_collection = None,

            Command::AddServer(origin) => {
                if !self.servers.contains_key(&origin) {
                    self.servers.insert(
                        origin.clone(),
                        Server::new(runtime, egui_ctx, origin.clone()),
                    );
                } else {
                    re_log::warn!(
                        "Tried to add pre-existing sever at {:?}",
                        origin.to_string()
                    );
                }
            }

            Command::RemoveServer(origin) => {
                self.servers.remove(&origin);
            }
        }
    }

    pub fn server_panel_ui(&mut self, ui: &mut egui::Ui) {
        ui.panel_content(|ui| {
            ui.panel_title_bar_with_buttons(
                "Servers",
                Some("These are the currently connected Redap servers."),
                |ui| {
                    if ui
                        .small_icon_button(&re_ui::icons::ADD)
                        .on_hover_text("Add a server")
                        .clicked()
                    {
                        self.add_server_modal_ui.open();
                    }
                },
            );
        });

        egui::ScrollArea::both()
            .id_salt("servers_scroll_area")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.panel_content(|ui| {
                    re_ui::list_item::list_item_scope(ui, "server panel", |ui| {
                        self.server_list_ui(ui);
                    });
                });
            });
    }

    fn server_list_ui(&self, ui: &mut egui::Ui) {
        self.with_ctx(|ctx| {
            for server in self.servers.values() {
                server.panel_ui(ctx, ui);
            }
        });
    }

    pub fn ui(&mut self, viewer_ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        self.add_server_modal_ui(ui);

        //TODO(ab): we should display something even if no catalog is currently selected.

        if let Some(selected_collection) = self.selected_collection.as_ref() {
            for server in self.servers.values() {
                let collection = server.find_collection(*selected_collection);

                if let Some(collection) = collection {
                    self.with_ctx(|ctx| {
                        super::collection_ui::collection_ui(
                            viewer_ctx,
                            ctx,
                            ui,
                            &server.origin,
                            collection,
                        );
                    });

                    return;
                }
            }
        }
    }

    fn add_server_modal_ui(&mut self, ui: &egui::Ui) {
        //TODO(ab): borrow checker doesn't let me use `with_ctx()` here, I should find a better way
        let ctx = Context {
            command_sender: &self.command_sender,
            selected_collection: &self.selected_collection,
        };

        self.add_server_modal_ui.ui(&ctx, ui);
    }

    #[inline]
    fn with_ctx<R>(&self, func: impl FnOnce(&Context<'_>) -> R) -> R {
        let ctx = Context {
            command_sender: &self.command_sender,
            selected_collection: &self.selected_collection,
        };

        func(&ctx)
    }
}
