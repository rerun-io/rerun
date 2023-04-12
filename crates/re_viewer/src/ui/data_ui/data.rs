use egui::Vec2;

use re_format::format_f32;
use re_log_types::{
    component_types::ColorRGBA,
    component_types::{LineStrip2D, LineStrip3D, Mat3x3, Rect2D, Vec2D, Vec3D, Vec4D},
    Pinhole, Rigid3, Transform, ViewCoordinates,
};

use crate::ui::UiVerbosity;

use super::DataUi;

/// Default number of ui points to show a number.
const DEFAULT_NUMBER_WIDTH: f32 = 52.0;

impl DataUi for [u8; 4] {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        let [r, g, b, a] = self;
        let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
        let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
        ui.painter().rect_stroke(
            response.rect,
            1.0,
            ui.visuals().widgets.noninteractive.fg_stroke,
        );
        response.on_hover_text(format!("Color #{r:02x}{g:02x}{b:02x}{a:02x}"));
    }
}

impl DataUi for ColorRGBA {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        let [r, g, b, a] = self.to_array();
        let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
        ui.painter().rect_stroke(
            response.rect,
            1.0,
            ui.visuals().widgets.noninteractive.fg_stroke,
        );
        response.on_hover_text(format!("Color #{r:02x}{g:02x}{b:02x}{a:02x}"));
    }
}

impl DataUi for Transform {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            Transform::Unknown => {
                ui.label("Unknown transform");
            }
            Transform::Rigid3(rigid3) => rigid3.data_ui(ctx, ui, verbosity, query),
            Transform::Pinhole(pinhole) => pinhole.data_ui(ctx, ui, verbosity, query),
        }
    }
}

impl DataUi for ViewCoordinates {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label(format!("ViewCoordinates: {}", self.describe()));
            }
            UiVerbosity::All | UiVerbosity::Reduced => {
                ui.label(self.describe());
            }
        }
    }
}

impl DataUi for Rigid3 {
    #[allow(clippy::only_used_in_recursion)]
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label("Rigid 3D transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiVerbosity::All, query);
                });
            }

            UiVerbosity::All | UiVerbosity::Reduced => {
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
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label("Pinhole transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiVerbosity::All, query);
                });
            }

            UiVerbosity::All | UiVerbosity::Reduced => {
                let Pinhole {
                    image_from_cam: image_from_view,
                    resolution,
                } = self;

                ui.vertical(|ui| {
                    ui.label("Pinhole transform:");
                    ui.indent("pinole", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("resolution:");
                            if let Some(re_log_types::component_types::Vec2D([x, y])) = resolution {
                                ui.monospace(format!("{x}x{y}"));
                            } else {
                                ui.weak("(none)");
                            }
                        });

                        ui.label("image from view:");
                        ui.indent("image_from_view", |ui| {
                            image_from_view.data_ui(ctx, ui, verbosity, query);
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
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
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

impl DataUi for Vec2D {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for Vec3D {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for Rect2D {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.label(match self {
            Rect2D::XYWH(Vec4D([top, left, width, height]))
            | Rect2D::YXHW(Vec4D([left, top, height, width])) => {
                format!("top: {top}, left: {left}, width: {width}, height: {height}")
            }
            Rect2D::XYXY(Vec4D([left, top, right, bottom]))
            | Rect2D::YXYX(Vec4D([top, left, bottom, right])) => {
                format!("top: {top}, left: {left}, right: {right}, bottom: {bottom}")
            }
            Rect2D::XCYCWH(Vec4D([center_x, center_y, width, height])) => {
                format!(
                    "center: {}, width: {width}, height: {height}",
                    Vec2D([*center_x, *center_y])
                )
            }
            Rect2D::XCYCW2H2(Vec4D([center_x, center_y, half_width, half_height])) => {
                format!(
                    "center: {}, half-width: {half_width}, half-height: {half_height}",
                    Vec2D([*center_x, *center_y])
                )
            }
        })
        .on_hover_text(format!("area: {}", self.width() * self.height()));
    }
}

impl DataUi for LineStrip2D {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small | UiVerbosity::Reduced => {
                ui.label(format!("{} positions", self.0.len()));
            }
            UiVerbosity::All => {
                use egui_extras::{Column, TableBuilder};
                TableBuilder::new(ui)
                    .resizable(true)
                    .vscroll(true)
                    .auto_shrink([false, true])
                    .max_scroll_height(100.0)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .columns(Column::initial(DEFAULT_NUMBER_WIDTH).clip(true), 2)
                    .header(re_ui::ReUi::table_header_height(), |mut header| {
                        re_ui::ReUi::setup_table_header(&mut header);
                        header.col(|ui| {
                            ui.label("x");
                        });
                        header.col(|ui| {
                            ui.label("y");
                        });
                    })
                    .body(|mut body| {
                        re_ui::ReUi::setup_table_body(&mut body);
                        let row_height = re_ui::ReUi::table_line_height();
                        body.rows(row_height, self.0.len(), |index, mut row| {
                            if let Some(pos) = self.0.get(index) {
                                row.col(|ui| {
                                    ui.label(format_f32(pos.x()));
                                });
                                row.col(|ui| {
                                    ui.label(format_f32(pos.y()));
                                });
                            }
                        });
                    });
            }
        }
    }
}

impl DataUi for LineStrip3D {
    fn data_ui(
        &self,
        _ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        match verbosity {
            UiVerbosity::Small | UiVerbosity::Reduced => {
                ui.label(format!("{} positions", self.0.len()));
            }
            UiVerbosity::All => {
                use egui_extras::{Column, TableBuilder};
                TableBuilder::new(ui)
                    .resizable(true)
                    .vscroll(true)
                    .auto_shrink([false, true])
                    .max_scroll_height(100.0)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .columns(Column::initial(DEFAULT_NUMBER_WIDTH).clip(true), 3)
                    .header(re_ui::ReUi::table_header_height(), |mut header| {
                        re_ui::ReUi::setup_table_header(&mut header);
                        header.col(|ui| {
                            ui.label("x");
                        });
                        header.col(|ui| {
                            ui.label("y");
                        });
                        header.col(|ui| {
                            ui.label("z");
                        });
                    })
                    .body(|mut body| {
                        re_ui::ReUi::setup_table_body(&mut body);
                        let row_height = re_ui::ReUi::table_line_height();
                        body.rows(row_height, self.0.len(), |index, mut row| {
                            if let Some(pos) = self.0.get(index) {
                                row.col(|ui| {
                                    ui.label(format_f32(pos.x()));
                                });
                                row.col(|ui| {
                                    ui.label(format_f32(pos.y()));
                                });
                                row.col(|ui| {
                                    ui.label(format_f32(pos.z()));
                                });
                            }
                        });
                    });
            }
        }
    }
}
