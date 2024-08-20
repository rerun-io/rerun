use egui::Ui;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_format::format_f32;
use re_types::blueprint::components::VisualBounds2D;
use re_types::components::{LineStrip2D, LineStrip3D};
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;

/// Default number of ui points to show a number.
const DEFAULT_NUMBER_WIDTH: f32 = 52.0;

impl DataUi for re_types::datatypes::Mat3x3 {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
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
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui_layout.data_label(ui, self.to_string());
    }
}

impl DataUi for re_types::datatypes::Vec3D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui_layout.data_label(ui, self.to_string());
    }
}

impl DataUi for re_types::datatypes::Vec4D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui_layout.data_label(ui, self.to_string());
    }
}

impl DataUi for re_types::datatypes::UVec2D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui_layout.data_label(ui, self.to_string());
    }
}

impl DataUi for re_types::datatypes::UVec3D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui_layout.data_label(ui, self.to_string());
    }
}

impl DataUi for re_types::datatypes::UVec4D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        ui_layout.data_label(ui, self.to_string());
    }
}

impl DataUi for VisualBounds2D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut Ui,
        ui_layout: UiLayout,
        _query: &LatestAtQuery,
        _db: &EntityDb,
    ) {
        ui_layout.data_label(ui, self.to_string());
    }
}

impl DataUi for LineStrip2D {
    fn data_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List | UiLayout::Tooltip => {
                ui_layout.label(ui, format!("{} positions", self.0.len()));
            }
            UiLayout::SelectionPanelLimitHeight | UiLayout::SelectionPanelFull => {
                use egui_extras::Column;
                ui_layout
                    .table(ui)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .columns(Column::initial(DEFAULT_NUMBER_WIDTH).clip(true), 2)
                    .header(re_ui::DesignTokens::table_header_height(), |mut header| {
                        re_ui::DesignTokens::setup_table_header(&mut header);
                        header.col(|ui| {
                            ui.label("x");
                        });
                        header.col(|ui| {
                            ui.label("y");
                        });
                    })
                    .body(|mut body| {
                        re_ui::DesignTokens::setup_table_body(&mut body);
                        let row_height = re_ui::DesignTokens::table_line_height();
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
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        match ui_layout {
            UiLayout::List | UiLayout::Tooltip => {
                ui_layout.label(ui, format!("{} positions", self.0.len()));
            }
            UiLayout::SelectionPanelFull | UiLayout::SelectionPanelLimitHeight => {
                use egui_extras::Column;
                ui_layout
                    .table(ui)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .columns(Column::initial(DEFAULT_NUMBER_WIDTH).clip(true), 3)
                    .header(re_ui::DesignTokens::table_header_height(), |mut header| {
                        re_ui::DesignTokens::setup_table_header(&mut header);
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
                        re_ui::DesignTokens::setup_table_body(&mut body);
                        let row_height = re_ui::DesignTokens::table_line_height();
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
