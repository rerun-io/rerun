use re_arrow_store::{ComponentBucket, ComponentTable, IndexBucket, IndexTable};
use re_data_store::Timeline;
use re_format::format_number;

use crate::ViewerContext;

/// Provides a debug view into the raw [`re_arrow_store::DataStore`] structures
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DataStoreView {}

impl DataStoreView {
    pub(crate) fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        let store = &ctx.log_db.obj_db.arrow_store;

        ui.vertical(|ui| {
            egui::Frame {
                inner_margin: re_ui::ReUi::view_padding().into(),
                ..egui::Frame::default()
            }
            .show(ui, |ui| {
                self.index_tables(store, ctx, ui);
            });

            egui::Frame {
                inner_margin: re_ui::ReUi::view_padding().into(),
                ..egui::Frame::default()
            }
            .show(ui, |ui| {
                self.component_tables(store, ctx, ui);
            });
        });
    }

    fn index_tables(
        &mut self,
        store: &re_arrow_store::DataStore,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
    ) {
        let indices = store.iter_indices();
        ui.label(format!("{} index tables", format_number(indices.len())));
        ui.separator();
        egui::ScrollArea::horizontal()
            .id_source("index_tables_scroller")
            .auto_shrink([true; 2])
            .show(ui, |ui| {
                egui::Grid::new("Index Tables")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        for ((timeline, obj_path), index_table) in indices {
                            self.index_table(ctx, &timeline, index_table, ui);
                        }
                    });
            });
    }

    fn index_table(
        &self,
        ctx: &mut ViewerContext<'_>,
        timeline: &Timeline,
        index_table: &IndexTable,
        ui: &mut egui::Ui,
    ) {
        ui.label(timeline.name().as_str());

        let _response = ui
            .horizontal(|ui| {
                // Add some spacing to match CollapsingHeader:
                ui.spacing_mut().item_spacing.x = 0.0;
                ctx.obj_path_button(ui, index_table.entity_path());
            })
            .response;

        let buckets = index_table.iter_buckets();
        egui::CollapsingHeader::new(format!("{} Buckets", format_number(buckets.len())))
            .id_source(timeline)
            .default_open(false)
            .show(ui, |ui| {
                for bucket in buckets {
                    self.index_bucket(bucket, ui);
                }
            });
        ui.end_row();
    }

    fn index_bucket(&self, bucket: &IndexBucket, ui: &mut egui::Ui) {
        use egui_extras::{Column, TableBuilder};

        ui.label(bucket.formatted_time_range());

        let row_height = re_ui::ReUi::table_line_height();
        let (names, cols) = bucket.named_indices();

        let displayers: Vec<_> = cols
            .iter()
            .map(|col| arrow2::array::get_display(col, "null"))
            .collect();

        TableBuilder::new(ui)
            .max_scroll_height(400.0)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .resizable(true)
            .columns(Column::auto().clip(true).at_least(50.0), names.len())
            .header(re_ui::ReUi::table_header_height(), |mut header| {
                re_ui::ReUi::setup_table_header(&mut header);
                for name in &names {
                    header.col(|ui| {
                        ui.strong(name);
                    });
                }
            })
            .body(|mut body| {
                re_ui::ReUi::setup_table_body(&mut body);
                body.rows(row_height, cols[0].len(), |row_idx, mut row| {
                    for disp in &displayers {
                        let mut string = String::new();
                        (disp)(&mut string, row_idx).unwrap();
                        row.col(|ui| {
                            ui.label(string);
                        });
                    }
                });
            });
    }

    fn component_tables(
        &mut self,
        store: &re_arrow_store::DataStore,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
    ) {
        let components = store.iter_components();
        ui.label(format!(
            "{} component tables",
            format_number(components.len())
        ));
        ui.separator();
        egui::ScrollArea::horizontal()
            .id_source("component_tables_scroller")
            .auto_shrink([true; 2])
            .show(ui, |ui| {
                egui::Grid::new("Component Tables")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        for (name, component_table) in components {
                            self.component_table(ctx, name.as_str(), component_table, ui);
                        }
                    });
            });
    }

    fn component_table(
        &self,
        ctx: &mut ViewerContext<'_>,
        name: &str,
        component_table: &ComponentTable,
        ui: &mut egui::Ui,
    ) {
        let _response = ui
            .horizontal(|ui| {
                // Add some spacing to match CollapsingHeader:
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.strong(component_table.name());
            })
            .response;

        let buckets = component_table.iter_buckets();
        egui::CollapsingHeader::new(format!("{} Buckets", format_number(buckets.len())))
            .id_source(name)
            .default_open(false)
            .show(ui, |ui| {
                for bucket in buckets {
                    self.component_bucket(ctx, bucket, ui);
                }
            });
        ui.end_row();
    }

    fn component_bucket(
        &self,
        ctx: &mut ViewerContext<'_>,
        bucket: &ComponentBucket,
        ui: &mut egui::Ui,
    ) {
        use egui_extras::{Column, TableBuilder};

        //ui.label(bucket.formatted_time_range());

        let row_height = re_ui::ReUi::table_line_height();
        let data = bucket.data();
        let displayer = arrow2::array::get_display(data.as_ref(), "null");

        TableBuilder::new(ui)
            .striped(true)
            .max_scroll_height(400.0)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .resizable(true)
            .columns(Column::auto().clip(true).at_least(50.0), 1)
            .header(re_ui::ReUi::table_header_height(), |mut header| {
                re_ui::ReUi::setup_table_header(&mut header);
                header.col(|ui| {
                    ui.strong(bucket.name());
                    ctx.data_type_button(ui, data.data_type());
                });
            })
            .body(|mut body| {
                re_ui::ReUi::setup_table_body(&mut body);
                body.rows(row_height, data.len(), |row_idx, mut row| {
                    let mut string = String::new();
                    (displayer)(&mut string, row_idx).unwrap();
                    row.col(|ui| {
                        ui.label(string);
                    });
                });
            });
    }
}
