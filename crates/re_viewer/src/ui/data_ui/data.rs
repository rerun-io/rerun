use egui::Vec2;
use re_log_types::{
    field_types::ColorRGBA, field_types::Mat3x3, Arrow3D, Data, DataVec, Pinhole, Rigid3,
    Transform, ViewCoordinates,
};

use crate::ui::Preview;

use super::{image::format_tensor_shape, DataUi};

/// Previously `data_ui::data_ui()`
impl DataUi for Data {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: crate::ui::Preview,
    ) -> egui::Response {
        match self {
            Data::Bool(value) => ui.label(value.to_string()),
            Data::I32(value) => ui.label(value.to_string()),
            Data::F32(value) => ui.label(value.to_string()),
            Data::F64(value) => ui.label(value.to_string()),
            Data::Color(value) => value.data_ui(ctx, ui, preview),
            Data::String(string) => ui.label(format!("{string:?}")),

            Data::Vec2([x, y]) => ui.label(format!("[{x:.1}, {y:.1}]")),
            Data::BBox2D(bbox) => ui.label(format!(
                "BBox2D(min: [{:.1} {:.1}], max: [{:.1} {:.1}])",
                bbox.min[0], bbox.min[1], bbox.max[0], bbox.max[1]
            )),

            Data::Vec3([x, y, z]) => ui.label(format!("[{x:.3}, {y:.3}, {z:.3}]")),
            Data::Box3(_) => ui.label("3D box"),
            Data::Mesh3D(_) => ui.label("3D mesh"),
            Data::Arrow3D(Arrow3D { origin, vector }) => {
                let [x, y, z] = origin.0;
                let [v0, v1, v2] = vector.0;
                ui.label(format!(
                    "Arrow3D(origin: [{x:.1},{y:.1},{z:.1}], vector: [{v0:.1},{v1:.1},{v2:.1}])"
                ))
            }
            Data::Transform(transform) => match preview {
                Preview::Small | Preview::Specific(_) => ui.monospace("Transform"),
                Preview::Medium => DataUi::data_ui(transform, ctx, ui, preview),
            },
            Data::ViewCoordinates(coordinates) => match preview {
                Preview::Small | Preview::Specific(_) => {
                    ui.label(format!("ViewCoordinates: {}", coordinates.describe()))
                }
                Preview::Medium => coordinates.data_ui(ctx, ui, preview),
            },
            Data::AnnotationContext(context) => match preview {
                Preview::Small | Preview::Specific(_) => ui.monospace("AnnotationContext"),
                Preview::Medium => DataUi::data_ui(context, ctx, ui, preview),
            },
            Data::Tensor(tensor) => {
                let tensor_view = ctx.cache.image.get_view(tensor, ctx.render_ctx);

                ui.horizontal_centered(|ui| {
                    let max_width = match preview {
                        Preview::Small => 32.0,
                        Preview::Medium => 128.0,
                        Preview::Specific(height) => height,
                    };

                    if let Some(retained_img) = tensor_view.retained_img {
                        retained_img
                            .show_max_size(ui, Vec2::new(4.0 * max_width, max_width))
                            .on_hover_ui(|ui| {
                                retained_img.show_max_size(ui, Vec2::splat(400.0));
                            });
                    }

                    ui.vertical(|ui| {
                        ui.set_min_width(100.0);
                        ui.label(format!("dtype: {}", tensor.dtype()));
                        ui.label(format!("shape: {}", format_tensor_shape(tensor.shape())));
                    });
                })
                .response
            }

            Data::ObjPath(obj_path) => ctx.obj_path_button(ui, obj_path),

            Data::DataVec(data_vec) => DataUi::data_ui(data_vec, ctx, ui, preview),
        }
    }

    fn detailed_data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        if let Data::Tensor(tensor) = self {
            tensor.data_ui(ctx, ui, preview)
        } else {
            self.data_ui(ctx, ui, Preview::Medium)
        }
    }
}

/// Previously `color_field_ui()`
impl DataUi for [u8; 4] {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        let [r, g, b, a] = self;
        let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
        let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
        ui.painter().rect_stroke(
            response.rect,
            1.0,
            ui.visuals().widgets.noninteractive.fg_stroke,
        );
        response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
    }
}

/// Previously `color_field_ui()`
impl DataUi for ColorRGBA {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        let [r, g, b, a] = self.to_array();
        let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
        ui.painter().rect_stroke(
            response.rect,
            1.0,
            ui.visuals().widgets.noninteractive.fg_stroke,
        );
        response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
    }
}

/// Previously `data_vec_ui()`
impl DataUi for DataVec {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        ui.label(format!("{} x {:?}", self.len(), self.element_data_type(),))
    }
}

/// Previously `transform_ui()`
impl DataUi for Transform {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        match self {
            Transform::Unknown => ui.label("Unknown"),
            Transform::Rigid3(rigid3) => rigid3.data_ui(ctx, ui, preview),
            Transform::Pinhole(pinhole) => pinhole.data_ui(ctx, ui, preview),
        }
    }
}

/// Previously `view_coordinates_ui()`
impl DataUi for ViewCoordinates {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        ui.label(self.describe())
    }
}

/// Previously `rigid3_ui()`
impl DataUi for Rigid3 {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        let pose = self.parent_from_child(); // TODO(emilk): which one to show?
        let rotation = pose.rotation();
        let translation = pose.translation();

        ui.vertical(|ui| {
            ui.label("Rigid3");
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
        })
        .response
    }
}

/// Previously `pinhole_ui()`
impl DataUi for Pinhole {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        let Pinhole {
            image_from_cam: image_from_view,
            resolution,
        } = self;

        ui.vertical(|ui| {
            ui.label("Pinhole");
            ui.indent("pinole", |ui| {
                egui::Grid::new("pinole").num_columns(2).show(ui, |ui| {
                    ui.label("image from view");
                    image_from_view.data_ui(ctx, ui, preview);
                    ui.end_row();

                    ui.label("resolution");
                    ui.monospace(format!("{resolution:?}"));
                    ui.end_row();
                });
            });
        })
        .response
    }
}

/// Previously `mat3_ui()`
impl DataUi for Mat3x3 {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _preview: Preview,
    ) -> egui::Response {
        egui::Grid::new("mat3")
            .num_columns(3)
            .show(ui, |ui| {
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
            })
            .response
    }
}
