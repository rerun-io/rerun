//! The `DataUi` trait and implementations provide methods for representing Rerun data objects and
//! types in `egui`.
//!

use itertools::Itertools;
use re_log_types::msg_bundle::ComponentBundle;
use re_log_types::{PathOp, TimePoint};

use crate::misc::ViewerContext;

use super::Preview;

mod context;
mod data;
pub(crate) mod image;
mod log_msg;
mod object;
mod path;

/// Types implementing `DataUi` can draw themselves with a `ViewerContext` and `egui::Ui`.
pub(crate) trait DataUi {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response;

    fn detailed_data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        self.data_ui(ctx, ui, preview)
    }
}

// ----------------------------------------------------------------------------

/// Previously: `time_point_ui()`
impl DataUi for TimePoint {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        ui.vertical(|ui| {
            egui::Grid::new("time_point").num_columns(2).show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (timeline, value) in self.iter() {
                    ctx.timeline_button(ui, timeline);
                    ui.label(": ");
                    ctx.time_button(ui, timeline, *value);
                    ui.end_row();
                }
            });
        })
        .response
    }
}

// TODO(jleibs): Better ArrowMsg view
/// Previously `logged_arrow_data_ui()`
impl DataUi for [ComponentBundle] {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        // TODO(john): more handling
        ui.label(format!(
            "Arrow Payload of {:?}",
            self.iter().map(|bundle| &bundle.name).collect_vec()
        ))
    }
}

/// Previously `path_op_ui()`
impl DataUi for PathOp {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        match self {
            PathOp::ClearFields(obj_path) => ui.label(format!("ClearFields: {obj_path}")),
            PathOp::ClearRecursive(obj_path) => ui.label(format!("ClearRecursive: {obj_path}")),
        }
    }
}
