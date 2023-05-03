use std::collections::BTreeMap;

use egui::{Color32, RichText};

use re_data_store::{EntityPath, Timeline};
use re_log_types::TimePoint;

use crate::ViewerContext;

use super::{NodeGraphEntry, SceneNodeGraph};
// --- Main view ---

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ViewNodeGraphState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// NodeGraph entry window however they please when the time cursor isn't moving.
    latest_time: i64,

    pub filters: ViewNodeGraphFilters,

    monospace: bool,
}

impl ViewNodeGraphState {
    pub fn selection_ui(&mut self, re_ui: &re_ui::ReUi, ui: &mut egui::Ui) {
        crate::profile_function!();
        re_log::info!("Holda from node graph");
    }
}

pub(crate) fn view_node_graph(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewNodeGraphState,
    scene: &SceneNodeGraph,
) -> egui::Response {
    crate::profile_function!();

    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        if ui.button("Button text").clicked() {
            re_log::info!("Holda from node graph");
        }
    })
    .response
}

// --- Filters ---

// TODO(cmc): implement "body contains <value>" filter.
// TODO(cmc): beyond filters, it'd be nice to be able to swap columns at some point.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ViewNodeGraphFilters {
    // Column filters: which columns should be visible?
    // Timelines are special: each one has a dedicated column.
    pub col_timelines: BTreeMap<Timeline, bool>,
    pub col_entity_path: bool,
    pub col_log_level: bool,

    // Row filters: which rows should be visible?
    pub row_entity_paths: BTreeMap<EntityPath, bool>,
    pub row_log_levels: BTreeMap<String, bool>,
}

impl Default for ViewNodeGraphFilters {
    fn default() -> Self {
        Self {
            col_entity_path: true,
            col_log_level: true,
            col_timelines: Default::default(),
            row_entity_paths: Default::default(),
            row_log_levels: Default::default(),
        }
    }
}

impl ViewNodeGraphFilters {
    pub fn is_entity_path_visible(&self, entity_path: &EntityPath) -> bool {
        self.row_entity_paths
            .get(entity_path)
            .copied()
            .unwrap_or(true)
    }

    pub fn is_log_level_visible(&self, level: &str) -> bool {
        self.row_log_levels.get(level).copied().unwrap_or(true)
    }

    // Checks whether new values are available for any of the filters, and updates everything
    // accordingly.
    fn update(&mut self, ctx: &mut ViewerContext<'_>, NodeGraph_entries: &[NodeGraphEntry]) {
        crate::profile_function!();
    }
}
