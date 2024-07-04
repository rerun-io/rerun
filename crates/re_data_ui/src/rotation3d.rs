use re_types::{
    components,
    datatypes::{self, RotationAxisAngle},
};
use re_viewer_context::{UiLayout, ViewerContext};

use crate::DataUi;

impl DataUi for components::Rotation3D {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
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
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        match self {
            Self::Quaternion(q) => {
                let [x, y, z, w] = q.xyzw();
                ui_layout.data_label(
                    ui,
                    format!("Quaternion XYZW: [{x:.2} {y:.2} {z:.2} {w:.2}]"),
                );
            }
            Self::AxisAngle(RotationAxisAngle { axis, angle }) => {
                match ui_layout {
                    UiLayout::List => {
                        // TODO(#6315): should be mixed label/data formatting
                        ui_layout.label(ui, format!("angle: {angle}, axis: {axis}"));
                    }
                    _ => {
                        egui::Grid::new("axis_angle").num_columns(2).show(ui, |ui| {
                            ui.label("axis");
                            axis.data_ui(ctx, ui, ui_layout, query, db);
                            ui.end_row();

                            ui.label("angle");
                            ui.label(angle.to_string());
                            ui.end_row();
                        });
                    }
                }
            }
        }
    }
}
