use egui::Vec2;

use re_format::format_f32;
use re_types::components::{Color, LineStrip2D, LineStrip3D, ViewCoordinates};
use re_viewer_context::{UiLayout, ViewerContext};

use super::{table_for_ui_layout, DataUi};

/// Default number of ui points to show a number.
const DEFAULT_NUMBER_WIDTH: f32 = 52.0;

impl DataUi for [u8; 4] {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
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
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
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
        ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List => {
                ui.label(self.describe_short())
                    .on_hover_text(self.describe());
            }
            UiLayout::SelectionPanelFull
            | UiLayout::SelectionPanelLimitHeight
            | UiLayout::Tooltip => {
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
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
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
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for re_types::datatypes::Vec3D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for re_types::datatypes::Vec4D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for re_types::datatypes::UVec2D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for re_types::datatypes::UVec3D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for re_types::datatypes::UVec4D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui.label(self.to_string());
    }
}

impl DataUi for LineStrip2D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List | UiLayout::Tooltip => {
                ui.label(format!("{} positions", self.0.len()));
            }
            UiLayout::SelectionPanelLimitHeight | UiLayout::SelectionPanelFull => {
                use egui_extras::Column;
                table_for_ui_layout(ui_layout, ui)
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
        ui_layout: UiLayout,
        _query: &re_data_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List | UiLayout::Tooltip => {
                ui.label(format!("{} positions", self.0.len()));
            }
            UiLayout::SelectionPanelFull | UiLayout::SelectionPanelLimitHeight => {
                use egui_extras::Column;
                table_for_ui_layout(ui_layout, ui)
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
