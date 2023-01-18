//! The `DataUi` trait and implementations provide methods for representing Rerun data objects and
//! types in `egui`.
//!

use itertools::Itertools;
use re_log_types::msg_bundle::ComponentBundle;
use re_log_types::{PathOp, TimePoint};

use crate::misc::ViewerContext;

mod component;
mod context;
mod data;
mod data_path;
pub(crate) mod image;
mod log_msg;
mod msg_id;
mod object;

/// Controls how large we show the data in [`DataUi`].
#[derive(Clone, Copy, Debug)]
pub(crate) enum Preview {
    /// Keep it very small, e.g. to fit into a table of many values.
    Small,

    /// As large as you want.
    Medium,

    /// At most this height
    MaxHeight(f32),
}

/// Types implementing [`DataUi`] can draw themselves with a [`ViewerContext`] and [`egui::Ui`].
pub(crate) trait DataUi {
    fn data_ui(&self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, preview: Preview);
}

// ----------------------------------------------------------------------------

impl DataUi for TimePoint {
    fn data_ui(&self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, _preview: Preview) {
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

// TODO(jleibs): Better ArrowMsg view
impl DataUi for [ComponentBundle] {
    fn data_ui(&self, _ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, _preview: Preview) {
        // TODO(john): more handling
        ui.label(format!(
            "Arrow Payload of {:?}",
            self.iter().map(|bundle| &bundle.name).collect_vec()
        ));
    }
}

impl DataUi for PathOp {
    fn data_ui(&self, _ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, _preview: Preview) {
        match self {
            PathOp::ClearFields(obj_path) => ui.label(format!("ClearFields: {obj_path}")),
            PathOp::ClearRecursive(obj_path) => ui.label(format!("ClearRecursive: {obj_path}")),
        };
    }
}
