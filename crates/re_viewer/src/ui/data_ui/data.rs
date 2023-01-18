use egui::Vec2;
use re_log_types::{
    field_types::ColorRGBA, field_types::Mat3x3, Arrow3D, Data, DataVec, Pinhole, Rigid3,
    Transform, ViewCoordinates,
};

use crate::ui::Preview;

use super::DataUi;

impl DataUi for Data {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: crate::ui::Preview,
    ) {
        match self {
            Data::Bool(value) => {
                ui.label(value.to_string());
            }
            Data::I32(value) => {
                ui.label(value.to_string());
            }
            Data::F32(value) => {
                ui.label(value.to_string());
            }
            Data::F64(value) => {
                ui.label(value.to_string());
            }
            Data::Color(value) => value.data_ui(ctx, ui, preview),
            Data::String(string) => {
                ui.label(format!("{string:?}"));
            }

            Data::Vec2([x, y]) => {
                ui.label(format!("[{x:.1}, {y:.1}]"));
            }
            Data::BBox2D(bbox) => {
                ui.label(format!(
                    "BBox2D(min: [{:.1} {:.1}], max: [{:.1} {:.1}])",
                    bbox.min[0], bbox.min[1], bbox.max[0], bbox.max[1]
                ));
            }

            Data::Vec3([x, y, z]) => {
                ui.label(format!("[{x:.3}, {y:.3}, {z:.3}]"));
            }
            Data::Box3(_) => {
                ui.label("3D box");
            }
            Data::Mesh3D(_) => {
                ui.label("3D mesh");
            }
            Data::Arrow3D(Arrow3D { origin, vector }) => {
                let [x, y, z] = origin.0;
                let [v0, v1, v2] = vector.0;
                ui.label(format!(
                    "Arrow3D(origin: [{x:.1},{y:.1},{z:.1}], vector: [{v0:.1},{v1:.1},{v2:.1}])"
                ));
            }
            Data::Transform(transform) => transform.data_ui(ctx, ui, preview),
            Data::ViewCoordinates(coordinates) => coordinates.data_ui(ctx, ui, preview),
            Data::AnnotationContext(context) => context.data_ui(ctx, ui, preview),
            Data::Tensor(tensor) => tensor.data_ui(ctx, ui, preview),

            Data::ObjPath(obj_path) => {
                ctx.obj_path_button(ui, obj_path);
            }

            Data::DataVec(data_vec) => data_vec.data_ui(ctx, ui, preview),
        }
    }
}

impl DataUi for [u8; 4] {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) {
        let [r, g, b, a] = self;
        let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
        let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
        ui.painter().rect_stroke(
            response.rect,
            1.0,
            ui.visuals().widgets.noninteractive.fg_stroke,
        );
        response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a));
    }
}

impl DataUi for ColorRGBA {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) {
        let [r, g, b, a] = self.to_array();
        let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
        ui.painter().rect_stroke(
            response.rect,
            1.0,
            ui.visuals().widgets.noninteractive.fg_stroke,
        );
        response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a));
    }
}

impl DataUi for DataVec {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) {
        ui.label(format!("{} x {:?}", self.len(), self.element_data_type()));
    }
}

impl DataUi for Transform {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) {
        match self {
            Transform::Unknown => {
                ui.label("Unknown transform");
            }
            Transform::Rigid3(rigid3) => rigid3.data_ui(ctx, ui, preview),
            Transform::Pinhole(pinhole) => pinhole.data_ui(ctx, ui, preview),
        }
    }
}

impl DataUi for ViewCoordinates {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) {
        match preview {
            Preview::Small | Preview::MaxHeight(_) => {
                ui.label(format!("ViewCoordinates: {}", self.describe()));
            }
            Preview::Medium => {
                ui.label(self.describe());
            }
        }
    }
}

impl DataUi for Rigid3 {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) {
        match preview {
            Preview::Small | Preview::MaxHeight(_) => {
                ui.label("Rigid 3D transform").on_hover_ui(|ui| {
                    self.data_ui(_ctx, ui, Preview::Medium);
                });
            }

            Preview::Medium => {
                let pose = self.parent_from_child(); // TODO(emilk): which one to show?
                let rotation = pose.rotation();
                let translation = pose.translation();

                ui.vertical(|ui| {
                    ui.label("Rigid 3D transform:");
                    ui.indent("rigid3", |ui| {
                        egui::Grid::new("rigid3").num_columns(2).show(ui, |ui| {
                            ui.label("rotation");
                            ui.monospace(format!("{rotation:?}"));
                            ui.end_row();

                            ui.label("translation");
                            ui.monospace(format!("{translation:?}"));
                            ui.end_row();
                        });
                    });
                });
            }
        }
    }
}

impl DataUi for Pinhole {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) {
        match preview {
            Preview::Small | Preview::MaxHeight(_) => {
                ui.label("Pinhole transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, Preview::Medium);
                });
            }

            Preview::Medium => {
                let Pinhole {
                    image_from_cam: image_from_view,
                    resolution,
                } = self;

                ui.vertical(|ui| {
                    ui.label("Pinhole transform:");
                    ui.indent("pinole", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("resolution:");
                            if let Some(re_log_types::field_types::Vec2D([x, y])) = resolution {
                                ui.monospace(format!("{x}x{y}"));
                            } else {
                                ui.weak("(none)");
                            }
                        });

                        ui.label("image from view:");
                        ui.indent("image_from_view", |ui| {
                            image_from_view.data_ui(ctx, ui, preview);
                        });
                    });
                });
            }
        }
    }
}

impl DataUi for Mat3x3 {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) {
        egui::Grid::new("mat3").num_columns(3).show(ui, |ui| {
            ui.monospace(self[0][0].to_string());
            ui.monospace(self[1][0].to_string());
            ui.monospace(self[2][0].to_string());
            ui.end_row();

            ui.monospace(self[0][1].to_string());
            ui.monospace(self[1][1].to_string());
            ui.monospace(self[2][1].to_string());
            ui.end_row();

            ui.monospace(self[0][2].to_string());
            ui.monospace(self[1][2].to_string());
            ui.monospace(self[2][2].to_string());
            ui.end_row();
        });
    }
}
