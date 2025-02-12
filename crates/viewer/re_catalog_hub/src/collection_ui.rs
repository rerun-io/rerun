use arrow::array::ArrayRef;
use arrow::{array::RecordBatch, datatypes::SchemaRef};
use egui::Ui;
use egui_table::{CellInfo, HeaderCellInfo};
use re_ui::UiLayout;
use re_viewer_context::ViewerContext;

use super::hub::{Command, RecordingCollection};

pub fn collection_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    collection: &RecordingCollection,
) -> Vec<Command> {
    let mut commands = vec![];

    let schema = {
        let Some(recording_batch) = collection.collection.first() else {
            //TODO: make that nicer
            ui.label("empty");
            return commands;
        };

        recording_batch.schema()
    };

    // The table id mainly drives column widths, along with the id of each column.
    //TODO: this needs to be fine-tuned for the column cache to behave correctly.
    let table_id_salt = egui::Id::new("__collection_ui__");

    let num_rows = collection
        .collection
        .iter()
        .map(|record_batch| record_batch.num_rows() as u64)
        .sum();

    let mut table_delegate = CollectionTableDelegate {
        ctx,
        record_batches: &collection.collection,
        schema: schema.clone(),
        num_rows,
    };

    egui::Frame::new().inner_margin(5.0).show(ui, |ui| {
        if ui.button("Close").clicked() {
            commands.push(Command::DeselectCollection);
        }

        egui_table::Table::new()
            .id_salt(table_id_salt)
            .columns(
                schema
                    .fields
                    .iter()
                    .map(|field| {
                        egui_table::Column::new(200.0)
                            .resizable(true)
                            .id(egui::Id::new(field.name()))
                    })
                    .collect::<Vec<_>>(),
            )
            .headers(vec![egui_table::HeaderRow::new(
                re_ui::DesignTokens::table_header_height(),
            )])
            .num_rows(num_rows)
            .show(ui, &mut table_delegate);
    });

    commands
}

struct CollectionTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    record_batches: &'a Vec<RecordBatch>,
    schema: SchemaRef,
    num_rows: u64,
}

impl egui_table::TableDelegate for CollectionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut Ui, cell: &HeaderCellInfo) {
        let name = self.schema.fields[cell.group_index].name().as_str();
        let name = name.strip_prefix("rerun_").unwrap_or(name);

        ui.strong(name);
    }

    fn cell_ui(&mut self, ui: &mut Ui, cell: &CellInfo) {
        // find record batch
        let mut row_index = cell.row_nr as usize;

        for record_batch in self.record_batches {
            let row_count = record_batch.num_rows();
            if row_index < row_count {
                // this is the one
                let column = record_batch.column(cell.col_nr);

                if column.is_null(row_index) {
                    ui.label("-");
                } else {
                    re_ui::arrow_ui(ui, UiLayout::List, &column.slice(row_index, 1));
                }
            } else {
                row_index -= row_count;
            }
        }
    }

    fn default_row_height(&self) -> f32 {
        re_ui::DesignTokens::table_line_height()
    }
}
