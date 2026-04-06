use ahash::HashMap;
use arrow::array::ArrayRef;
use re_chunk::RowId;
use re_chunk_store::external::re_chunk::Chunk;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, StoreId, TimePoint};
use re_sdk_types::ComponentDescriptor;
use re_ui::ContextExt as _;

use crate::drag_and_drop::DragAndDropPayload;
use crate::time_control::TimeControlCommand;
use crate::{
    ActiveStoreContext, AppOptions, ApplicationSelectionState, CommandSender, ComponentUiRegistry,
    DragAndDropManager, FallbackProviderRegistry, Item, ItemCollection, Route, StorageContext,
    StoreHub, SystemCommand, SystemCommandSender as _, TableStores, TimeControl, ViewClassRegistry,
};

/// Application context that is shared across all parts of the viewer.
///
/// This context, in difference to [`crate::ViewerContext`] can exist for
/// any arbitrary state of the viewer. And not only when there is an open
/// recording.
pub struct AppContext<'a> {
    /// Set during tests (e.g. snapshot tests).
    ///
    /// Used to hide non-deterministic UI elements such as the current time.
    pub is_test: bool,

    pub memory_limit: re_memory::MemoryLimit,

    /// Global options for the whole viewer.
    pub app_options: &'a AppOptions,

    /// Runtime info about components and archetypes.
    pub reflection: &'a re_types_core::reflection::Reflection,

    /// The [`egui::Context`].
    pub egui_ctx: &'a egui::Context,

    /// The global `re_renderer` context, holds on to all GPU resources.
    pub render_ctx: &'a re_renderer::RenderContext,

    /// Interface for sending commands back to the app
    pub command_sender: &'a CommandSender,

    /// Registry of authenticated redap connections
    pub connection_registry: &'a re_redap_client::ConnectionRegistryHandle,

    /// All loaded recordings, blueprints, tables, etc.
    pub storage_context: &'a StorageContext<'a>,

    /// The currently active store and blueprint, if any.
    ///
    /// This is `None` if the current [`Route`] is not pointing to a recording.
    pub active_store_context: Option<&'a ActiveStoreContext<'a>>,

    /// How to display components.
    pub component_ui_registry: &'a ComponentUiRegistry,

    /// Registry of all known classes of views.
    pub view_class_registry: &'a ViewClassRegistry,

    /// Defaults for components in various contexts.
    pub component_fallback_registry: &'a FallbackProviderRegistry,

    /// The current route of the viewer.
    pub route: &'a Route,

    /// Selection & hovering state.
    pub selection_state: &'a ApplicationSelectionState,

    /// Item that got focused on the last frame if any.
    ///
    /// The focused item is cleared every frame, but views may react with side-effects
    /// that last several frames.
    pub focused_item: &'a Option<crate::Item>,

    /// Helper object to manage drag-and-drop operations.
    pub drag_and_drop_manager: &'a DragAndDropManager,

    /// The time control for the active recording, if any.
    pub active_time_ctrl: Option<&'a TimeControl>,

    /// Where we are getting our data from.
    pub connected_receivers: &'a re_log_channel::LogReceiverSet,

    /// Are we logged in to rerun cloud?
    pub auth_context: Option<&'a AuthContext>,
}

pub struct AuthContext {
    pub email: String,
    pub org_name: Option<String>,
}

impl AppContext<'_> {
    pub fn logged_in(&self) -> bool {
        self.auth_context.is_some()
    }

    pub fn selection_state(&self) -> &ApplicationSelectionState {
        self.selection_state
    }

    /// Returns the current selection.
    pub fn selection(&self) -> &ItemCollection {
        self.selection_state.selected_items()
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &ItemCollection {
        self.selection_state.hovered_items()
    }

    /// Returns if this item should be displayed as selected or not.
    ///
    /// This does not always line up with [`Self::selection`], if we
    /// are currently loading something that will be prioritized here.
    pub fn is_selected_or_loading(&self, item: &crate::Item) -> bool {
        if let Route::Loading(source) = self.route {
            if let crate::Item::DataSource(other_source) = item {
                source.is_same_ignoring_uri_fragments(other_source)
            } else {
                false
            }
        } else {
            self.selection().contains_item(item)
        }
    }

    /// The current active Redap entry id, if any.
    pub fn active_redap_entry(&self) -> Option<re_log_types::EntryId> {
        match self.route {
            Route::RedapEntry(entry) => Some(entry.entry_id),
            _ => None,
        }
    }

    /// The current active local table, if any.
    pub fn active_table_id(&self) -> Option<&re_log_types::TableId> {
        match self.route {
            Route::LocalTable(table_id) => Some(table_id),
            _ => None,
        }
    }

    pub fn tokens(&self) -> &'static re_ui::DesignTokens {
        self.egui_ctx.tokens()
    }

    /// How to configure the renderer.
    #[inline]
    pub fn render_mode(&self) -> re_renderer::RenderMode {
        if self.is_test {
            re_renderer::RenderMode::Deterministic
        } else {
            re_renderer::RenderMode::Beautiful
        }
    }

    /// The [`StoreHub`].
    pub fn store_hub(&self) -> &StoreHub {
        self.storage_context.hub
    }

    /// All loaded recordings, blueprints, etc.
    pub fn store_bundle(&self) -> &re_entity_db::StoreBundle {
        self.storage_context.bundle
    }

    /// All loaded tables.
    pub fn table_stores(&self) -> &TableStores {
        self.storage_context.tables
    }

    /// Item that got focused on the last frame if any.
    pub fn focused_item(&self) -> Option<&crate::Item> {
        self.focused_item.as_ref()
    }

    /// Helper object to manage drag-and-drop operations.
    pub fn drag_and_drop_manager(&self) -> &DragAndDropManager {
        self.drag_and_drop_manager
    }

    /// The active recording, if any.
    pub fn active_recording(&self) -> Option<&EntityDb> {
        self.active_store_context.map(|ctx| ctx.recording)
    }

    /// The active blueprint, if any.
    pub fn active_blueprint(&self) -> Option<&EntityDb> {
        self.active_store_context.map(|ctx| ctx.blueprint)
    }

    /// The time control for the active recording, if any.
    pub fn active_time_ctrl(&self) -> Option<&TimeControl> {
        self.active_time_ctrl
    }

    /// Helper function to send [`TimeControlCommand`]s for the active recording.
    pub fn send_time_commands_to_active_recording(
        &self,
        commands: impl IntoIterator<Item = TimeControlCommand>,
    ) {
        let commands: Vec<_> = commands.into_iter().collect();
        if !commands.is_empty() {
            if let Some(store_ctx) = self.active_store_context {
                self.command_sender
                    .send_system(SystemCommand::TimeControlCommands {
                        store_id: store_ctx.recording.store_id().clone(),
                        time_commands: commands,
                    });
            } else {
                re_log::debug_warn_once!("Time command ignored - no active recording");
            }
        }
    }

    /// Consistently handle the selection, hover, drag start interactions for a given set of items.
    pub fn handle_select_hover_drag_interactions(
        &self,
        response: &egui::Response,
        interacted_items: impl Into<ItemCollection>,
        draggable: bool,
    ) {
        let mut interacted_items = interacted_items.into();

        if let Some(store_ctx) = self.active_store_context
            && let Some(time_ctrl) = self.active_time_ctrl
        {
            interacted_items = interacted_items
                .into_mono_instance_path_items(store_ctx.recording, &time_ctrl.current_query());
        }
        let selection_state = self.selection_state();

        if response.hovered() {
            selection_state.set_hovered(interacted_items.clone());
        }

        let single_selected = self.selection().single_item() == interacted_items.single_item();

        if single_selected && self.selection_state().selection_changed().is_some() {
            response.scroll_to_me(None);
        }

        if draggable && response.drag_started() {
            let mut selected_items = selection_state.selected_items().clone();
            let is_already_selected = interacted_items
                .iter()
                .all(|(item, _)| selected_items.contains_item(item));

            let is_cmd_held = response.ctx.input(|i| i.modifiers.command);

            let dragged_items = if !is_already_selected && is_cmd_held {
                selected_items.extend(interacted_items);
                self.command_sender
                    .send_system(SystemCommand::set_selection(selected_items.clone()));
                selected_items
            } else if !is_already_selected {
                interacted_items
            } else {
                selected_items
            };

            let items_may_be_dragged = self
                .drag_and_drop_manager
                .are_items_draggable(&dragged_items);

            let payload = if items_may_be_dragged {
                DragAndDropPayload::from_items(&dragged_items)
            } else {
                DragAndDropPayload::Invalid
            };

            egui::DragAndDrop::set_payload(&response.ctx, payload);
        } else if response.clicked() {
            if response.double_clicked()
                && let Some(item) = interacted_items.first_item()
            {
                let item = if let Item::DataResult(data_result) = item {
                    interacted_items = Item::DataResult(data_result.as_entity_all()).into();
                    interacted_items
                        .first_item()
                        .expect("That item was just added")
                } else {
                    item
                };

                self.command_sender
                    .send_system(SystemCommand::SetFocus(item.clone()));
            }

            let modifiers = response.ctx.input(|i| i.modifiers);

            if !modifiers.shift {
                if modifiers.command {
                    let mut toggle_items_set: HashMap<_, _> = interacted_items
                        .iter()
                        .map(|(item, ctx)| (item.clone(), ctx.clone()))
                        .collect();

                    let mut new_selection = selection_state.selected_items().clone();

                    new_selection.retain(|item, ctx| {
                        if let Some(new_ctx) = toggle_items_set.get(item) {
                            if new_ctx == ctx || new_ctx.is_none() {
                                toggle_items_set.remove(item);
                                false
                            } else {
                                true
                            }
                        } else {
                            true
                        }
                    });

                    for (item, ctx) in new_selection.iter_mut() {
                        if let Some(new_ctx) = toggle_items_set.get(item) {
                            *ctx = new_ctx.clone();
                            toggle_items_set.remove(item);
                        }
                    }

                    new_selection.extend(
                        interacted_items
                            .into_iter()
                            .filter(|(item, _)| toggle_items_set.contains_key(item)),
                    );

                    self.command_sender
                        .send_system(SystemCommand::set_selection(new_selection));
                } else {
                    self.command_sender
                        .send_system(SystemCommand::set_selection(interacted_items));
                }
            }
        }
    }

    /// Reverts to the default route.
    pub fn revert_to_default_route(&self) {
        self.command_sender.send_system(SystemCommand::ResetRoute);
    }

    /// Append an array to the given store.
    pub fn append_array_to_store(
        &self,
        store_id: StoreId,
        timepoint: TimePoint,
        entity_path: EntityPath,
        component_descr: ComponentDescriptor,
        array: ArrayRef,
    ) {
        let chunk = match Chunk::builder(entity_path)
            .with_row(RowId::new(), timepoint, [(component_descr, array)])
            .build()
        {
            Ok(chunk) => chunk,
            Err(err) => {
                re_log::error_once!("Failed to create Chunk: {err}");
                return;
            }
        };

        self.command_sender
            .send_system(SystemCommand::AppendToStore(store_id, vec![chunk]));
    }
}
