use ahash::HashMap;
use re_chunk_store::LatestAtQuery;
use re_entity_db::entity_db::EntityDb;
use re_log_types::{EntryId, TableId};
use re_query::StorageEngineReadGuard;
use re_sdk_types::ViewClassIdentifier;
use re_ui::ContextExt as _;
use re_ui::list_item::ListItem;

use crate::command_sender::{SelectionSource, SetSelection};
use crate::component_fallbacks::FallbackProviderRegistry;
use crate::drag_and_drop::DragAndDropPayload;
use crate::query_context::DataQueryResult;
use crate::time_control::TimeControlCommand;
use crate::{
    AppContext, AppOptions, ApplicationSelectionState, CommandSender, ComponentUiRegistry,
    DisplayMode, DragAndDropManager, IndicatedEntities, Item, ItemCollection, PerVisualizerType,
    PerVisualizerTypeInViewClass, StoreContext, StoreHub, SystemCommand, SystemCommandSender as _,
    TimeControl, ViewClassRegistry, ViewId, VisualizableEntities,
};

/// Common things needed to view a specific recording with a specific blueprint.
pub struct ViewerContext<'a> {
    /// App context shared across all parts of the viewer.
    pub app_ctx: AppContext<'a>,

    /// Registry of all known classes of views.
    pub view_class_registry: &'a ViewClassRegistry,

    /// Defaults for components in various contexts.
    pub component_fallback_registry: &'a FallbackProviderRegistry,

    /// For each visualizer, the set of entities that are known to have all its required components.
    // TODO(andreas): This could have a generation id, allowing to update heuristics entities etc. more lazily.
    pub visualizable_entities_per_visualizer: &'a PerVisualizerType<VisualizableEntities>,

    /// For each visualizer, the set of entities with relevant archetypes.
    ///
    /// TODO(andreas): Should we always do the intersection with `maybe_visualizable_entities_per_visualizer`
    ///                 or are we ever interested in a (definitely-)non-visualizable but archetype-matching entity?
    pub indicated_entities_per_visualizer: &'a PerVisualizerType<IndicatedEntities>,

    /// All the query results for this frame.
    pub query_results: &'a HashMap<ViewId, DataQueryResult>,

    /// UI config for the current recording (found in [`EntityDb`]).
    pub time_ctrl: &'a TimeControl,

    /// UI config for the current blueprint.
    pub blueprint_time_ctrl: &'a TimeControl,

    /// The blueprint query used for resolving blueprint in this frame
    pub blueprint_query: &'a LatestAtQuery,

    /// Selection & hovering state.
    pub selection_state: &'a ApplicationSelectionState,

    /// Item that got focused on the last frame if any.
    ///
    /// The focused item is cleared every frame, but views may react with side-effects
    /// that last several frames.
    pub focused_item: &'a Option<crate::Item>,

    /// Helper object to manage drag-and-drop operations.
    pub drag_and_drop_manager: &'a DragAndDropManager,

    /// Where we are getting our data from.
    pub connected_receivers: &'a re_log_channel::LogReceiverSet,

    /// The active recording and blueprint.
    pub store_context: &'a StoreContext<'a>,
}

// Forwarding of `AppContext` methods to `ViewerContext`. Leaving this as a
// separate block for easier refactoring (i.e. macros) in the future.
impl ViewerContext<'_> {
    /// Global options for the whole viewer.
    pub fn app_options(&self) -> &AppOptions {
        self.app_ctx.app_options
    }

    pub fn tokens(&self) -> &'static re_ui::DesignTokens {
        self.egui_ctx().tokens()
    }

    /// Runtime info about components and archetypes.
    pub fn reflection(&self) -> &re_types_core::reflection::Reflection {
        self.app_ctx.reflection
    }

    /// How to display components.
    pub fn component_ui_registry(&self) -> &ComponentUiRegistry {
        self.app_ctx.component_ui_registry
    }

    /// Registry of all known classes of views.
    pub fn view_class_registry(&self) -> &ViewClassRegistry {
        self.view_class_registry
    }

    /// The [`egui::Context`].
    pub fn egui_ctx(&self) -> &egui::Context {
        self.app_ctx.egui_ctx
    }

    /// The global `re_renderer` context, holds on to all GPU resources.
    pub fn render_ctx(&self) -> &re_renderer::RenderContext {
        self.app_ctx.render_ctx
    }

    /// How to configure the renderer
    #[inline]
    pub fn render_mode(&self) -> re_renderer::RenderMode {
        if self.app_ctx.is_test {
            re_renderer::RenderMode::Deterministic
        } else {
            re_renderer::RenderMode::Beautiful
        }
    }

    /// Interface for sending commands back to the app
    pub fn command_sender(&self) -> &CommandSender {
        self.app_ctx.command_sender
    }

    /// The active display mode
    pub fn display_mode(&self) -> &crate::DisplayMode {
        self.app_ctx.display_mode
    }

    /// The [`StoreHub`].
    pub fn store_hub(&self) -> &StoreHub {
        self.app_ctx.storage_context.hub
    }

    /// All loaded recordings, blueprints, etc.
    pub fn store_bundle(&self) -> &re_entity_db::StoreBundle {
        self.app_ctx.storage_context.bundle
    }

    /// All loaded tables.
    pub fn table_stores(&self) -> &crate::TableStores {
        self.app_ctx.storage_context.tables
    }
}

impl ViewerContext<'_> {
    /// The active recording.
    #[inline]
    pub fn recording(&self) -> &EntityDb {
        self.store_context.recording
    }

    /// The active blueprint.
    #[inline]
    pub fn blueprint_db(&self) -> &re_entity_db::EntityDb {
        self.store_context.blueprint
    }

    /// The `StorageEngine` for the active recording.
    #[inline]
    pub fn recording_engine(&self) -> StorageEngineReadGuard<'_> {
        self.store_context.recording.storage_engine()
    }

    /// The `StorageEngine` for the active blueprint.
    #[inline]
    pub fn blueprint_engine(&self) -> StorageEngineReadGuard<'_> {
        self.store_context.blueprint.storage_engine()
    }

    /// The `StoreId` of the active recording.
    #[inline]
    pub fn store_id(&self) -> &re_log_types::StoreId {
        self.store_context.recording.store_id()
    }

    /// Returns the current selection.
    pub fn selection(&self) -> &ItemCollection {
        self.selection_state.selected_items()
    }

    /// Returns if this item should be displayed as selected or not.
    ///
    /// This does not always line up with [`Self::selection`], if we
    /// are currently loading something that will be prioritized here.
    pub fn is_selected_or_loading(&self, item: &Item) -> bool {
        if let DisplayMode::Loading(source) = self.display_mode() {
            if let Item::DataSource(other_source) = item {
                source.is_same_ignoring_uri_fragments(other_source)
            } else {
                false
            }
        } else {
            self.selection().contains_item(item)
        }
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &ItemCollection {
        self.selection_state.hovered_items()
    }

    pub fn selection_state(&self) -> &ApplicationSelectionState {
        self.selection_state
    }

    /// The current active Redap entry id, if any.
    pub fn active_redap_entry(&self) -> Option<EntryId> {
        match self.display_mode() {
            DisplayMode::RedapEntry(entry) => Some(entry.entry_id),
            _ => None,
        }
    }

    /// The current active local table, if any.
    pub fn active_table_id(&self) -> Option<&TableId> {
        match self.display_mode() {
            DisplayMode::LocalTable(table_id) => Some(table_id),
            _ => None,
        }
    }

    pub fn current_query(&self) -> re_chunk_store::LatestAtQuery {
        self.time_ctrl.current_query()
    }

    /// Helper function to send a [`SystemCommand::TimeControlCommands`] command
    /// with the current store id.
    pub fn send_time_commands(&self, commands: impl IntoIterator<Item = TimeControlCommand>) {
        let commands: Vec<_> = commands.into_iter().collect();

        if !commands.is_empty() {
            self.command_sender()
                .send_system(SystemCommand::TimeControlCommands {
                    store_id: self.store_id().clone(),
                    time_commands: commands,
                });
        }
    }

    /// Consistently handle the selection, hover, drag start interactions for a given set of items.
    ///
    /// The `draggable` parameter controls whether a drag can be initiated from this item. When a UI
    /// element represents an [`crate::Item`], one must make the call whether this element should be
    /// meaningfully draggable by the users. This is ultimately a subjective decision, but some here
    /// are some guidelines:
    /// - Is there a meaningful destination for the dragged payload? For example, dragging stuff out
    ///   of a modal dialog is by definition meaningless.
    /// - Even if a drag destination exists, would that be obvious to the user?
    /// - Is it expected for that kind of UI element to be draggable? For example, buttons aren't
    ///   typically draggable.
    ///
    /// Drag vs. selection semantics:
    ///
    /// - When dragging an unselected item, that item only is dragged, and the selection is
    ///   unchanged…
    /// - …unless cmd/ctrl is held, in which case the item is added to the selection and the entire
    ///   selection is dragged.
    /// - When dragging a selected item, the entire selection is dragged as well.
    ///
    /// You might also want to call [`Self::handle_select_focus_sync`] to keep keyboard focus in
    /// sync with selection.
    pub fn handle_select_hover_drag_interactions(
        &self,
        response: &egui::Response,
        interacted_items: impl Into<ItemCollection>,
        draggable: bool,
    ) {
        let mut interacted_items = interacted_items
            .into()
            .into_mono_instance_path_items(self.recording(), &self.current_query());
        let selection_state = self.selection_state();

        if response.hovered() {
            selection_state.set_hovered(interacted_items.clone());
        }

        let single_selected = self.selection().single_item() == interacted_items.single_item();

        // If we were just selected, scroll into view
        if single_selected && self.selection_state().selection_changed().is_some() {
            response.scroll_to_me(None);
        }

        if draggable && response.drag_started() {
            let mut selected_items = selection_state.selected_items().clone();
            let is_already_selected = interacted_items
                .iter()
                .all(|(item, _)| selected_items.contains_item(item));

            let is_cmd_held = response.ctx.input(|i| i.modifiers.command);

            // see semantics description in the docstring
            let dragged_items = if !is_already_selected && is_cmd_held {
                selected_items.extend(interacted_items);
                self.command_sender()
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
                // Double click always selects the whole instance and nothing else.
                let item = if let Item::DataResult(data_result) = item {
                    interacted_items = Item::DataResult(data_result.as_entity_all()).into();
                    interacted_items
                        .first_item()
                        .expect("That item was just added")
                } else {
                    item
                };

                self.app_ctx
                    .command_sender
                    .send_system(crate::SystemCommand::SetFocus(item.clone()));
            }

            let modifiers = response.ctx.input(|i| i.modifiers);

            // Shift-clicking means extending the selection. This generally requires local context,
            // so we don't handle it here.
            if !modifiers.shift {
                if modifiers.command {
                    // Sends a command to select `ìnteracted_items` unless already selected in which case they get unselected.
                    // If however an object is already selected but now gets passed a *different* item context, it stays selected after all
                    // but with an updated context!

                    let mut toggle_items_set: HashMap<_, _> = interacted_items
                        .iter()
                        .map(|(item, ctx)| (item.clone(), ctx.clone()))
                        .collect();

                    let mut new_selection = selection_state.selected_items().clone();

                    // If an item was already selected with the exact same context remove it.
                    // If an item was already selected and loses its context, remove it.
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

                    // Update context for items that are remaining in the toggle_item_set:
                    for (item, ctx) in new_selection.iter_mut() {
                        if let Some(new_ctx) = toggle_items_set.get(item) {
                            *ctx = new_ctx.clone();
                            toggle_items_set.remove(item);
                        }
                    }

                    // Make sure we preserve the order - old items kept in same order, new items added to the end.
                    // Add the new items, unless they were toggling out existing items:
                    new_selection.extend(
                        interacted_items
                            .into_iter()
                            .filter(|(item, _)| toggle_items_set.contains_key(item)),
                    );

                    self.command_sender()
                        .send_system(SystemCommand::set_selection(new_selection));
                } else {
                    self.command_sender()
                        .send_system(SystemCommand::set_selection(interacted_items));
                }
            }
        }
    }

    /// Helper to synchronize item selection with egui focus.
    ///
    /// Call if _this_ is where the user would expect keyboard focus to be
    /// when the item is selected (e.g. blueprint tree for views, recording panel for recordings).
    pub fn handle_select_focus_sync(
        &self,
        response: &egui::Response,
        interacted_items: impl Into<ItemCollection>,
    ) {
        let interacted_items = interacted_items
            .into()
            .into_mono_instance_path_items(self.recording(), &self.current_query());

        // Focus -> Selection

        // We want the item to be selected if it was selected with arrow keys (in list_item)
        // but not when focused using e.g. the tab key.
        if ListItem::gained_focus_via_arrow_key(&response.ctx, response.id) {
            self.command_sender()
                .send_system(SystemCommand::SetSelection(
                    SetSelection::new(interacted_items.clone())
                        .with_source(SelectionSource::ListItemNavigation),
                ));
        }

        // Selection -> Focus

        let single_selected = self.selection().single_item() == interacted_items.single_item();
        if single_selected {
            // If selection changes, and a single item is selected, the selected item should
            // receive egui focus.
            // We don't do this if selection happened due to list item navigation to avoid
            // a feedback loop.
            let selection_changed = self
                .selection_state()
                .selection_changed()
                .is_some_and(|source| source != SelectionSource::ListItemNavigation);

            // If there is a single selected item and nothing is focused, focus that item.
            let nothing_focused = response.ctx.memory(|mem| mem.focused().is_none());

            if selection_changed || nothing_focused {
                response.request_focus();
            }
        }
    }

    /// Are we running inside the Safari browser?
    pub fn is_safari_browser(&self) -> bool {
        #![expect(clippy::unused_self)]

        #[cfg(target_arch = "wasm32")]
        fn is_safari_browser_inner() -> Option<bool> {
            use web_sys::wasm_bindgen::JsValue;
            let window = web_sys::window()?;
            Some(window.has_own_property(&JsValue::from("safari")))
        }

        #[cfg(not(target_arch = "wasm32"))]
        fn is_safari_browser_inner() -> Option<bool> {
            None
        }

        is_safari_browser_inner().unwrap_or(false)
    }

    /// This returns `true` if we have an active recording.
    ///
    /// It excludes the globally hardcoded welcome screen app ID.
    pub fn has_active_recording(&self) -> bool {
        self.recording().application_id() != &StoreHub::welcome_screen_app_id()
    }

    /// Reverts to the default display mode
    pub fn revert_to_default_display_mode(&self) {
        self.command_sender()
            .send_system(SystemCommand::ResetDisplayMode);
    }

    /// Iterates over all entities that are visualizeable for a given view class.
    ///
    /// This is a subset of [`Self::visualizable_entities_per_visualizer`], filtered to only include entities
    /// that are relevant for the visualizers used in the given view class.
    pub fn iter_visualizable_entities_for_view_class(
        &self,
        class: ViewClassIdentifier,
    ) -> impl Iterator<Item = (crate::ViewSystemIdentifier, &VisualizableEntities)> {
        let Some(view_class_entry) = self.view_class_registry().class_entry(class) else {
            return itertools::Either::Left(std::iter::empty());
        };

        itertools::Either::Right(
            self.visualizable_entities_per_visualizer
                .iter()
                .filter(|(viz_id, _entities)| {
                    view_class_entry.visualizer_system_ids.contains(viz_id)
                })
                .map(|(viz_id, entities)| (*viz_id, entities)),
        )
    }

    /// Like [`Self::iter_visualizable_entities_for_view_class`], but collects into a [`PerVisualizerTypeInViewClass`].
    pub fn collect_visualizable_entities_for_view_class(
        &self,
        view_class_identifier: ViewClassIdentifier,
    ) -> PerVisualizerTypeInViewClass<VisualizableEntities> {
        re_tracing::profile_function!();

        PerVisualizerTypeInViewClass {
            view_class_identifier,
            per_visualizer: self
                .iter_visualizable_entities_for_view_class(view_class_identifier)
                .map(|(viz_id, entities)| (viz_id, entities.clone()))
                .collect(),
        }
    }
}
