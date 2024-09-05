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
    display_record_batches: &[DisplayRecordBatch],
    query_timeline: Timeline,
    num_rows: u64,
) {
    let get_batch_and_index = |row: usize| -> (&DisplayRecordBatch, usize) {
        assert!(row < num_rows as usize);

        let mut row = row;
        for batch in display_record_batches {
            if row < batch.num_rows() {
                return (batch, row);
            }
            row -= batch.num_rows();
        }
        panic!("row out of bounds");
    };

    let header_ui = |mut row: egui_extras::TableRow<'_, '_>| {
        for column in schema {
            row.col(|ui| {
                ui.strong(column.short_name());
            });
        }
    };

    let row_ui = |mut row: egui_extras::TableRow<'_, '_>| {
        re_tracing::profile_scope!("row_ui");
        let (batch, batch_index) = get_batch_and_index(row.index());

        //TODO: wrong!
        let latest_at_query = LatestAtQuery::new(query_timeline, TimeInt::MAX);

        let row_id = RowId::ZERO;

        for column in batch.columns() {
            row.col(|ui| column.data_ui(ctx, ui, row_id, &latest_at_query, batch_index));
        }
    };

    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            egui::Frame {
                inner_margin: egui::Margin::same(5.0),
                ..Default::default()
            }
            .show(ui, |ui| {
                let mut table_builder = egui_extras::TableBuilder::new(ui)
                    .columns(
                        egui_extras::Column::auto_with_initial_suggestion(200.0).clip(true),
                        schema.len(),
                    )
                    .resizable(true)
                    .vscroll(true)
                    //TODO(ab): remove when https://github.com/emilk/egui/pull/4817 is merged/released
                    .max_scroll_height(f32::INFINITY)
                    .auto_shrink([false, false])
                    .striped(true);

                // if let Some(scroll_to_row) = scroll_to_row {
                //     table_builder =
                //         table_builder.scroll_to_row(scroll_to_row, Some(egui::Align::TOP));
                // }

                table_builder
                    .header(re_ui::DesignTokens::table_line_height(), header_ui)
                    .body(|body| {
                        body.rows(
                            re_ui::DesignTokens::table_line_height(),
                            //TODO: minor annoyance
                            num_rows as usize,
                            row_ui,
                        );
                    });
            });
        });
}
