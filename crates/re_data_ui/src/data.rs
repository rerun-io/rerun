use egui::Vec2;

use re_format::format_f32;
use re_types::components::{Color, LineStrip2D, LineStrip3D, MeshProperties, ViewCoordinates};
use re_viewer_context::{UiVerbosity, ViewerContext};

use super::{table_for_verbosity, DataUi};

/// Default number of ui points to show a number.
const DEFAULT_NUMBER_WIDTH: f32 = 52.0;

impl DataUi for [u8; 4] {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
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

impl DataUi for Color {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
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

impl DataUi for ViewCoordinates {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        match verbosity {
            UiVerbosity::Small => {
                ui.label(format!("ViewCoordinates: {}", self.describe()));
            }
            UiVerbosity::Full | UiVerbosity::LimitHeight | UiVerbosity::Reduced => {
                ui.label(self.describe());
            }
        }
    }
}

impl DataUi for re_types::datatypes::Mat3x3 {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        egui::Grid::new("mat3").num_columns(3).show(ui, |ui| {
            ui.monospace(self[0].to_string());
            ui.monospace(self[3].to_string());
            ui.monospace(self[6].to_string());
            ui.end_row();

            ui.monospace(self[1].to_string());
            ui.monospace(self[4].to_string());
            ui.monospace(self[7].to_string());
            ui.end_row();

            ui.monospace(self[2].to_string());
            ui.monospace(self[5].to_string());
            ui.monospace(self[8].to_string());
            ui.end_row();
        });
    }
}

impl DataUi for re_types::datatypes::Vec2D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for re_types::datatypes::Vec3D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for LineStrip2D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        match verbosity {
            UiVerbosity::Small | UiVerbosity::Reduced => {
                ui.label(format!("{} positions", self.0.len()));
            }
            UiVerbosity::LimitHeight | UiVerbosity::Full => {
                use egui_extras::Column;
                table_for_verbosity(verbosity, ui)
                    .resizable(true)
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
                        body.rows(row_height, self.0.len(), |mut row| {
                            if let Some(pos) = self.0.get(row.index()) {
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
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        match verbosity {
            UiVerbosity::Small | UiVerbosity::Reduced => {
                ui.label(format!("{} positions", self.0.len()));
            }
            UiVerbosity::Full | UiVerbosity::LimitHeight => {
                use egui_extras::Column;
                table_for_verbosity(verbosity, ui)
                    .resizable(true)
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
                        body.rows(row_height, self.0.len(), |mut row| {
                            if let Some(pos) = self.0.get(row.index()) {
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

impl DataUi for MeshProperties {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
        _store: &re_data_store::DataStore,
    ) {
        let show_optional_indices = |ui: &mut egui::Ui| {
            if let Some(indices) = self.indices.as_ref() {
                ui.label(format!(
                    "{} triangles",
                    re_format::format_uint(indices.len() / 3)
                ));
            } else {
                ui.weak("(empty)");
            }
        };

        match verbosity {
            UiVerbosity::Small | UiVerbosity::Reduced => {
                show_optional_indices(ui);
            }
            UiVerbosity::Full | UiVerbosity::LimitHeight => {
                egui::Grid::new("material").num_columns(2).show(ui, |ui| {
                    ui.label("triangles");
                    show_optional_indices(ui);
                    ui.end_row();
                });
            }
        }
    }
}
