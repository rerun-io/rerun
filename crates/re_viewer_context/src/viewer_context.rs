use re_data_store::log_db::LogDb;

use crate::{
    AppOptions, Caches, ComponentUiRegistry, Item, ItemCollection, SelectionState, TimeControl,
};

/// Common things needed by many parts of the viewer.
pub struct ViewerContext<'a> {
    /// Global options for the whole viewer.
    pub app_options: &'a mut AppOptions,

    /// Things that need caching.
    pub cache: &'a mut Caches,

    /// How to display components.
    pub component_ui_registry: &'a ComponentUiRegistry,

    /// The current recording.
    pub log_db: &'a LogDb,

    /// UI config for the current recording (found in [`LogDb`]).
    pub rec_cfg: &'a mut RecordingConfig,

    /// The look and feel of the UI.
    pub re_ui: &'a re_ui::ReUi,

    /// The global `re_renderer` context, holds on to all GPU resources.
    pub render_ctx: &'a mut re_renderer::RenderContext,
}

impl<'a> ViewerContext<'a> {
    /// Sets a single selection, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_single_selection(&mut self, item: Item) -> ItemCollection {
        self.rec_cfg.selection_state.set_single_selection(item)
    }

    /// Returns the current selection.
    pub fn selection(&self) -> &ItemCollection {
        self.rec_cfg.selection_state.current()
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &ItemCollection {
        self.rec_cfg.selection_state.hovered()
    }

    /// Set the hovered objects. Will be in [`Self::hovered`] on the next frame.
    pub fn set_hovered(&mut self, hovered: impl Iterator<Item = Item>) {
        self.rec_cfg.selection_state.set_hovered(hovered);
    }

    pub fn selection_state(&self) -> &SelectionState {
        &self.rec_cfg.selection_state
    }

    pub fn selection_state_mut(&mut self) -> &mut SelectionState {
        &mut self.rec_cfg.selection_state
    }

    /// The current time query, based on the current time control.
    pub fn current_query(&self) -> re_arrow_store::LatestAtQuery {
        self.rec_cfg.time_ctrl.current_query()
    }
}

// ----------------------------------------------------------------------------

/// UI config for the current recording (found in [`LogDb`]).
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RecordingConfig {
    /// The current time of the time panel, how fast it is moving, etc.
    pub time_ctrl: TimeControl,

    /// Selection & hovering state.
    pub selection_state: SelectionState,
}
