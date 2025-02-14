use egui_table::{CellInfo, HeaderCellInfo};

use re_sorbet::AnyColumnDescriptor;
use re_ui::UiExt;
use re_view_dataframe::display_record_batch::DisplayRecordBatch;
use re_viewer_context::external::re_log_types::Timeline;
use re_viewer_context::ViewerContext;

use super::hub::{Command, RecordingCollection};

pub fn collection_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    collection: &RecordingCollection,
) -> Vec<Command> {
    let mut commands = vec![];

    let sorbet_schema = {
        let Some(recording_batch) = collection.collection.first() else {
            ui.label(egui::RichText::new("This collection is empty").italics());
            return commands;
        };

        recording_batch.sorbet_schema()
    };

    // The table id mainly drives column widths, along with the id of each column.
    let table_id_salt = collection.collection_id.with("__collection_table__");

    let num_rows = collection
        .collection
        .iter()
        .map(|record_batch| record_batch.num_rows() as u64)
        .sum();

    let columns = sorbet_schema.columns.descriptors().collect::<Vec<_>>();

    let display_record_batches: Result<Vec<_>, _> = collection
        .collection
        .iter()
        .map(|sorbet_batch| {
            DisplayRecordBatch::try_new(
                sorbet_batch
                    .all_columns()
                    .map(|(desc, array)| (desc, array.clone())),
            )
        })
        .collect();

    let display_record_batches = match display_record_batches {
        Ok(display_record_batches) => display_record_batches,
        Err(err) => {
            //TODO: better error handling?
            ui.error_label(err.to_string());
            return commands;
        }
    };

    let mut table_delegate = CollectionTableDelegate {
        ctx,
        display_record_batches: &display_record_batches,
        selected_columns: &columns,
    };

    egui::Frame::new().inner_margin(5.0).show(ui, |ui| {
        if ui.button("Close").clicked() {
            commands.push(Command::DeselectCollection);
        }

        egui_table::Table::new()
            .id_salt(table_id_salt)
            .columns(
                columns
                    .iter()
                    .map(|field| {
                        egui_table::Column::new(200.0)
                            .resizable(true)
                            .id(egui::Id::new(field))
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
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    selected_columns: &'a Vec<AnyColumnDescriptor>,
}

impl egui_table::TableDelegate for CollectionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        ui.set_truncate_style();

        let name = self.selected_columns[cell.group_index].short_name();
        let name = name
            .strip_prefix("rerun_")
            .unwrap_or(name.as_str())
            .replace('_', " ");

        ui.strong(name);
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        // find record batch
        let mut row_index = cell.row_nr as usize;

        ui.set_truncate_style();

        for display_record_batch in self.display_record_batches {
            let row_count = display_record_batch.num_rows();
            if row_index < row_count {
                // this is the one
                let column = &display_record_batch.columns()[cell.col_nr];

                column.data_ui(
                    self.ctx,
                    ui,
                    //TODO: oh god
                    &re_viewer_context::external::re_chunk_store::LatestAtQuery::latest(
                        Timeline::new_sequence("unknown"),
                    ),
                    row_index,
                    None,
                );
            } else {
                row_index -= row_count;
            }
        }
    }

    fn default_row_height(&self) -> f32 {
        re_ui::DesignTokens::table_line_height()
    }
}
