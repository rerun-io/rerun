use re_format::format_number;

use re_arrow_store::{IndexBucket, IndexTable};
use re_data_store::{TimeInt, Timeline};

use crate::{Preview, ViewerContext};

/// Provides a debug view into the raw [`re_arrow_store::DataStore`] structures
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DataStoreView {}

impl DataStoreView {
    pub(crate) fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        let store = &ctx.log_db.obj_db.arrow_store;

        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            let indices = store.indices_iter();

            ui.label(format!("{} index tables", format_number(indices.len())));
            ui.separator();

            egui::ScrollArea::horizontal()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    egui::Grid::new("Index Tables")
                        .num_columns(3)
                        .striped(true)
                        .show(ui, |ui| {
                            for ((timeline, _), index_table) in indices {
                                self.table_index(ctx, timeline, index_table, ui);
                            }
                        });
                })
        });
    }
    fn table_index(
        &self,
        ctx: &mut ViewerContext<'_>,
        timeline: &Timeline,
        index_table: &IndexTable,
        ui: &mut egui::Ui,
    ) {
        let indent = ui.spacing().indent;

        ui.label(timeline.name().as_str());

        let response = ui
            .horizontal(|ui| {
                // Add some spacing to match CollapsingHeader:
                ui.spacing_mut().item_spacing.x = 0.0;
                ctx.obj_path_button(ui, &index_table.entity_path());
            })
            .response;

        let buckets = index_table.buckets_iter();
        egui::CollapsingHeader::new(format!("{} Buckets", format_number(buckets.len())))
            .default_open(false)
            .show(ui, |ui| {
                for (time, bucket) in buckets {
                    self.bucket(time, bucket, ui);
                }
            });
        ui.end_row();
    }

    fn bucket(&self, time: &TimeInt, bucket: &IndexBucket, ui: &mut egui::Ui) {
        ui.label(bucket.formatted_time_range());

        let df = bucket.as_frame().unwrap();
        foo(&df, ui);
    }
}

fn foo(df: &polars::prelude::DataFrame, ui: &mut egui::Ui) {
    use egui_extras::{Column, TableBuilder};

    TableBuilder::new(ui)
        .max_scroll_height(400.0)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .resizable(true)
        .columns(Column::auto().clip(true).at_least(50.0), df.width())
        .header(re_ui::ReUi::table_header_height(), |mut header| {
            re_ui::ReUi::setup_table_header(&mut header);
            for col in df.iter() {
                header.col(|ui| {
                    ui.strong(col.name());
                });
            }
        })
        .body(|mut body| {
            re_ui::ReUi::setup_table_body(&mut body);
        });
}
