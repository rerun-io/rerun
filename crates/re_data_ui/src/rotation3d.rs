use re_types::{
    components,
    datatypes::{self, Angle, RotationAxisAngle},
};
use re_viewer_context::{UiVerbosity, ViewerContext};

use crate::DataUi;

impl DataUi for components::Rotation3D {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        self.0.data_ui(ctx, ui, verbosity, query);
    }
}

impl DataUi for datatypes::Rotation3D {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            datatypes::Rotation3D::Quaternion(q) => {
                // TODO(andreas): Better formatting for quaternions.
                ui.label(format!("{q:?}"));
            }
            datatypes::Rotation3D::AxisAngle(RotationAxisAngle { axis, angle }) => {
                egui::Grid::new("axis_angle").num_columns(2).show(ui, |ui| {
                    ui.label("axis");
                    axis.data_ui(ctx, ui, verbosity, query);
                    ui.end_row();

                    ui.label("angle");
                    match angle {
                        Angle::Radians(v) => {
                            ui.label(format!("{}rad", re_format::format_f32(*v)));
                        }
                        Angle::Degrees(v) => {
                            // TODO(andreas): Convert to arc minutes/seconds for very small angles.
                            // That code should be in re_format!
                            ui.label(format!("{}°", re_format::format_f32(*v),));
                        }
                    }
                    ui.end_row();
                });
            }
        }
    }
}
