//! The `DataUi` trait and implementations provide methods for representing data using [`egui`].

use itertools::Itertools;
use re_log_types::{msg_bundle::ComponentBundle, PathOp, TimePoint};

use crate::misc::ViewerContext;

mod annotation_context;
mod component;
mod component_path;
mod component_ui_registry;
mod data;
mod entity_path;
pub(crate) mod image;
mod instance_path;
mod log_msg;
mod msg_id;

pub(crate) use component_ui_registry::ComponentUiRegistry;

/// Controls how mich space we use to show the data in [`DataUi`].
#[derive(Clone, Copy, Debug)]
pub enum UiVerbosity {
    /// Keep it small enough to fit on one row.
    Small,

    /// At most this height
    MaxHeight(f32),

    /// Display a reduced set, used for hovering.
    Reduced,

    /// Display everything, as large as you want. Used for selection panel.
    All,
}

/// Types implementing [`DataUi`] can draw themselves with a [`ViewerContext`] and [`egui::Ui`].
pub(crate) trait DataUi {
    /// If you need to lookup something in the data store, use the given query to do so.
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    );
}

// ----------------------------------------------------------------------------

impl DataUi for TimePoint {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.vertical(|ui| {
            egui::Grid::new("time_point").num_columns(2).show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (timeline, value) in self.iter() {
                    ctx.timeline_button_to(ui, format!("{}:", timeline.name()), timeline);
                    ctx.time_button(ui, timeline, *value);
                    ui.end_row();
                }
            });
        });
    }
}

impl DataUi for [ComponentBundle] {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        let mut sorted = self.to_vec();
        sorted.sort_by_key(|cb| cb.name);

        match verbosity {
            UiVerbosity::Small | UiVerbosity::MaxHeight(_) => {
                ui.label(sorted.iter().map(format_component_bundle).join(", "));
            }

            UiVerbosity::All | UiVerbosity::Reduced => {
                ui.vertical(|ui| {
                    for component_bundle in &sorted {
                        ui.label(format_component_bundle(component_bundle));
                    }
                });
            }
        }
    }
}

fn format_component_bundle(component_bundle: &ComponentBundle) -> String {
    let ComponentBundle { name, value } = component_bundle;

    use re_arrow_store::ArrayExt as _;
    let num_instances = value.get_child_length(0);

    // TODO(emilk): if there's only once instance, and the byte size is small, then deserialize and show the value.

    format!("{}x {}", num_instances, name.short_name())
}

impl DataUi for PathOp {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            PathOp::ClearComponents(entity_path) => {
                ui.label(format!("ClearComponents: {entity_path}"))
            }
            PathOp::ClearRecursive(entity_path) => {
                ui.label(format!("ClearRecursive: {entity_path}"))
            }
        };
    }
}
