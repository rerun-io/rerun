use re_types::{
    components,
    datatypes::{self, Angle, RotationAxisAngle},
};
use re_viewer_context::{UiLayout, ViewerContext};

use crate::{data_label_for_ui_layout, label_for_ui_layout, DataUi};

impl DataUi for components::Rotation3D {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        self.0.data_ui(ctx, ui, ui_layout, query, db);
    }
}

impl DataUi for datatypes::Rotation3D {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        match self {
            datatypes::Rotation3D::Quaternion(q) => {
                // TODO(andreas): Better formatting for quaternions.
                data_label_for_ui_layout(ui, ui_layout, format!("{q:?}"));
            }
            datatypes::Rotation3D::AxisAngle(RotationAxisAngle { axis, angle }) => {
                match ui_layout {
                    UiLayout::List => {
                        // TODO(#6315): should be mixed label/data formatting
                        label_for_ui_layout(ui, ui_layout, format!("angle: {angle}, axis: {axis}"));
                    }
                    _ => {
                        egui::Grid::new("axis_angle").num_columns(2).show(ui, |ui| {
                            ui.label("axis");
                            axis.data_ui(ctx, ui, ui_layout, query, db);
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
    }
}
