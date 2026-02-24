use crate::{
    AppOptions, ApplicationSelectionState, CommandSender, ComponentUiRegistry, DisplayMode,
    DragAndDropManager, ItemCollection, StorageContext,
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

    /// How to display components.
    pub component_ui_registry: &'a ComponentUiRegistry,

    /// The current display mode of the viewer.
    pub display_mode: &'a DisplayMode,

    /// Selection & hovering state.
    pub selection_state: &'a ApplicationSelectionState,

    /// Item that got focused on the last frame if any.
    ///
    /// The focused item is cleared every frame, but views may react with side-effects
    /// that last several frames.
    pub focused_item: &'a Option<crate::Item>,

    /// Helper object to manage drag-and-drop operations.
    pub drag_and_drop_manager: &'a DragAndDropManager,

    /// Are we logged in to rerun cloud?
    pub auth_context: Option<&'a AuthContext>,
}

pub struct AuthContext {
    pub email: String,
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
        if let DisplayMode::Loading(source) = self.display_mode {
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
        match self.display_mode {
            DisplayMode::RedapEntry(entry) => Some(entry.entry_id),
            _ => None,
        }
    }

    /// The current active local table, if any.
    pub fn active_table_id(&self) -> Option<&re_log_types::TableId> {
        match self.display_mode {
            DisplayMode::LocalTable(table_id) => Some(table_id),
            _ => None,
        }
    }
}
