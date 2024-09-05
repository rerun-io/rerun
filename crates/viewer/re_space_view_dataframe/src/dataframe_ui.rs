use std::collections::BTreeMap;

use egui::ahash::HashMap;

use re_chunk_store::external::re_chunk::external::arrow2::array::{
    Array as ArrowArray, ListArray, PrimitiveArray as ArrowPrimitiveArray, StructArray,
};
use re_chunk_store::{ColumnDescriptor, LatestAtQuery, RowId};
use re_dataframe::{LatestAtQueryHandle, RangeQueryHandle, RecordBatch};
use re_log_types::{EntityPath, TimeInt, Timeline, TimelineName};
use re_types_core::ComponentName;
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

use crate::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};

pub(crate) fn latest_at_dataframe_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_handle: LatestAtQueryHandle<'_>,
) {
    re_tracing::profile_function!();

    let schema = query_handle.schema();

    let num_rows = 1;
    let record_batch = query_handle.get();

    let display_record_batch = DisplayRecordBatch::try_new(&record_batch, schema);

    let display_record_batch = match display_record_batch {
        Ok(display_record_batch) => display_record_batch,
        Err(err) => {
            ui.error_label(&format!("{err}"));
            return;
        }
    };

    dataframe_ui(
        ctx,
        ui,
        schema,
        &[display_record_batch],
        query_handle.query().timeline,
        num_rows,
    );
}
pub(crate) fn range_dataframe_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_handle: RangeQueryHandle<'_>,
) {
    re_tracing::profile_function!();

    let schema = query_handle.schema();
    let num_rows = query_handle.num_rows();
    let record_batches = query_handle.get(0, num_rows);
    let total_batch_rows = record_batches
        .iter()
        .map(|batch| batch.num_rows())
        .sum::<usize>();
    if total_batch_rows != num_rows as usize {
        ui.error_label(&format!(
            "Row count mismatch: sum of record batch {total_batch_rows} (in {} batches) != query  {num_rows}", record_batches.len()
        ));
        return;
    }

    let display_record_batches: Result<Vec<_>, _> = record_batches
        .iter()
        .map(|batch| DisplayRecordBatch::try_new(batch, schema))
        .collect();

    let display_record_batches = match display_record_batches {
        Ok(display_record_batches) => display_record_batches,
        Err(err) => {
            ui.error_label(&format!("{err}"));
            return;
        }
    };

    dataframe_ui(
        ctx,
        ui,
        schema,
        &display_record_batches,
        query_handle.query().timeline,
        num_rows,
    );
}

fn dataframe_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    schema: &[ColumnDescriptor],
    display_record_batches: &[DisplayRecordBatch<'_>],
    query_timeline: Timeline,
    num_rows: u64,
) {
    re_tracing::profile_function!();

    // TODO(emilk): hierarchical header rows
    const NUM_HEADER_ROWS: u64 = 1;

    struct MyTableDelegate<'a> {
        ctx: &'a ViewerContext<'a>,
        schema: &'a [ColumnDescriptor],
        display_record_batches: &'a [DisplayRecordBatch<'a>],
        query_timeline: Timeline,

        batch_and_row_from_row_nr: BTreeMap<u64, (usize, usize)>,
    }

    impl<'a> MyTableDelegate<'a> {
        fn batch_and_index(&self, row: u64) -> (usize, usize) {
            let mut row = row as usize;
            for (batch_index, batch) in self.display_record_batches.iter().enumerate() {
                if row < batch.num_rows() {
                    return (batch_index, row);
                }
                row -= batch.num_rows();
            }
            panic!("row out of bounds");
        }
    }

    impl<'a> egui_table::TableDelegate for MyTableDelegate<'a> {
        fn prefetch_rows(&mut self, row_numbers: std::ops::Range<u64>) {
            re_tracing::profile_function!();

            for table_row_nr in row_numbers {
                if table_row_nr < NUM_HEADER_ROWS {
                    continue; // header row
                }
                let row_nr = table_row_nr - NUM_HEADER_ROWS; // ignore header rows
                self.batch_and_row_from_row_nr
                    .insert(row_nr, self.batch_and_index(row_nr));
            }
        }

        fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
            re_tracing::profile_function!();

            if cell.row_nr % 2 == 1 {
                // Paint stripes
                ui.painter()
                    .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
            }

            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(4.0, 0.0))
                .show(ui, |ui| {
                    if cell.row_nr < NUM_HEADER_ROWS {
                        // Header row
                        ui.strong(self.schema[cell.col_nr].short_name());
                        return;
                    }

                    let row_nr = cell.row_nr - NUM_HEADER_ROWS; // ignore header rows

                    //TODO: wrong!
                    let latest_at_query = LatestAtQuery::new(self.query_timeline, TimeInt::MAX);

                    let row_id = RowId::ZERO;

                    let (batch_nr, batch_index) = self.batch_and_row_from_row_nr[&row_nr]; // Will have been pre-fetched
                    let batch = &self.display_record_batches[batch_nr];
                    let column = &batch.columns()[cell.col_nr];

                    if ui.is_sizing_pass() {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    } else {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    }
                    column.data_ui(self.ctx, ui, row_id, &latest_at_query, batch_index);
                });
        }
    }
    let mut table_delegate = MyTableDelegate {
        ctx,
        schema,
        display_record_batches,
        query_timeline,

        batch_and_row_from_row_nr: Default::default(), // Will be filled during pre-fetch
    };

    let num_sticky_cols = schema
        .iter()
        .take_while(|cd| matches!(cd, ColumnDescriptor::Control(_) | ColumnDescriptor::Time(_)))
        .count();

    egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
        egui_table::Table {
            columns: vec![egui_table::Column::new(200.0, 0.0..=f32::INFINITY); schema.len()],
            num_sticky_cols,
            sticky_row_heights: vec![20.0],
            num_rows: NUM_HEADER_ROWS + num_rows,
            row_height: re_ui::DesignTokens::table_line_height(),
            ..Default::default()
        }
        .show(ui, &mut table_delegate);
    });
}
