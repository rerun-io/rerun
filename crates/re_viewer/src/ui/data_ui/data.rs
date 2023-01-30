use egui::Vec2;

use re_log_types::{
    component_types::ColorRGBA, component_types::Mat3x3, Pinhole, Rigid3, Transform,
    ViewCoordinates,
};

use crate::ui::UiVerbosity;

use super::DataUi;

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
        response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a));
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
        response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a));
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
            UiVerbosity::Small | UiVerbosity::MaxHeight(_) => {
                ui.label(format!("ViewCoordinates: {}", self.describe()));
            }
            UiVerbosity::Large => {
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
            UiVerbosity::Small | UiVerbosity::MaxHeight(_) => {
                ui.label("Rigid 3D transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiVerbosity::Large, query);
                });
            }

            UiVerbosity::Large => {
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
            UiVerbosity::Small | UiVerbosity::MaxHeight(_) => {
                ui.label("Pinhole transform").on_hover_ui(|ui| {
                    self.data_ui(ctx, ui, UiVerbosity::Large, query);
                });
            }

            UiVerbosity::Large => {
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
