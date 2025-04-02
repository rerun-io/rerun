use egui_table::{CellInfo, HeaderCellInfo};

use re_log_types::TimelineName;
use re_sorbet::{ColumnDescriptorRef, SorbetBatch};
use re_ui::UiExt as _;
use re_view_dataframe::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};
use re_viewer_context::{TableContext, ViewerContext};

pub fn table_ui(viewer_ctx: &ViewerContext<'_>, ui: &mut egui::Ui, context: &TableContext<'_>) {
    let batches = context.store.batches();

    let sorbet_schema = {
        let Some(sorbet_batch) = batches.first() else {
            ui.label(egui::RichText::new("This collection is empty").italics());
            return;
        };

        sorbet_batch.sorbet_schema()
    };

    // The table id mainly drives column widths, along with the id of each column.
    let table_id_salt = egui::Id::new(&context.table_id).with("__sorbet_batch_table__");

    let num_rows = batches
        .iter()
        .map(|record_batch| record_batch.num_rows() as u64)
        .sum();

    let columns = sorbet_schema.columns.descriptors().collect::<Vec<_>>();

    //TODO(ab): better column order?

    let display_record_batches: Result<Vec<_>, _> = batches
        .iter()
        .map(sorbet_batch_to_display_record_batch)
        .collect();

    let display_record_batches = match display_record_batches {
        Ok(display_record_batches) => display_record_batches,
        Err(err) => {
            //TODO(ab): better error handling?
            ui.error_label(err.to_string());
            return;
        }
    };

    let mut table_delegate = CollectionTableDelegate {
        ctx: viewer_ctx,
        display_record_batches: &display_record_batches,
        selected_columns: &columns,
    };

    egui::Frame::new().inner_margin(5.0).show(ui, |ui| {
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
}

fn sorbet_batch_to_display_record_batch(
    sorbet_batch: &SorbetBatch,
) -> Result<DisplayRecordBatch, DisplayRecordBatchError> {
    DisplayRecordBatch::try_new(
        sorbet_batch
            .all_columns()
            .map(|(desc, array)| (desc, array.clone())),
    )
}

struct CollectionTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    selected_columns: &'a Vec<ColumnDescriptorRef<'a>>,
}

impl egui_table::TableDelegate for CollectionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        ui.set_truncate_style();

        let name = self.selected_columns[cell.group_index].name();
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

                // TODO(#9029): it is _very_ unfortunate that we must provide a fake timeline, but
                // avoiding doing so needs significant refactoring work.
                column.data_ui(
                    self.ctx,
                    ui,
                    &re_viewer_context::external::re_chunk_store::LatestAtQuery::latest(
                        TimelineName::new("unknown"),
                    ),
                    row_index,
                    None,
                );

                break;
            } else {
                row_index -= row_count;
            }
        }
    }

    fn default_row_height(&self) -> f32 {
        re_ui::DesignTokens::table_line_height()
    }
}
