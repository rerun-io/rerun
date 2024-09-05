use std::collections::BTreeMap;

use re_chunk_store::{ColumnDescriptor, LatestAtQuery, RowId};
use re_dataframe::{LatestAtQueryHandle, RangeQueryHandle, RecordBatch};
use re_log_types::{TimeInt, Timeline};
use re_ui::UiExt as _;
use re_viewer_context::ViewerContext;

use crate::display_record_batch::DisplayRecordBatch;

pub(crate) fn latest_at_dataframe_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_handle: LatestAtQueryHandle<'_>,
) {
    re_tracing::profile_function!();

    dataframe_ui(ctx, ui, &QueryHandle::LatestAt(query_handle));
}
pub(crate) fn range_dataframe_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_handle: RangeQueryHandle<'_>,
) {
    re_tracing::profile_function!();

    dataframe_ui(ctx, ui, &QueryHandle::Range(query_handle));
}

enum QueryHandle<'a> {
    LatestAt(LatestAtQueryHandle<'a>),
    Range(RangeQueryHandle<'a>),
}

impl QueryHandle<'_> {
    fn schema(&self) -> &[ColumnDescriptor] {
        match self {
            QueryHandle::LatestAt(query_handle) => query_handle.schema(),
            QueryHandle::Range(query_handle) => query_handle.schema(),
        }
    }

    fn num_rows(&self) -> u64 {
        match self {
            QueryHandle::LatestAt(_) => 1,
            QueryHandle::Range(query_handle) => query_handle.num_rows(),
        }
    }

    fn get(&self, start: u64, num_rows: u64) -> Vec<RecordBatch> {
        match self {
            QueryHandle::LatestAt(query_handle) => {
                vec![query_handle.get()]
            }
            QueryHandle::Range(query_handle) => query_handle.get(start, num_rows),
        }
    }

    fn timeline(&self) -> Timeline {
        match self {
            QueryHandle::LatestAt(query_handle) => query_handle.query().timeline,
            QueryHandle::Range(query_handle) => query_handle.query().timeline,
        }
    }
}

fn dataframe_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, query_handle: &QueryHandle<'_>) {
    re_tracing::profile_function!();

    // TODO(emilk): hierarchical header rows
    const NUM_HEADER_ROWS: u64 = 1;

    struct MyTableDelegate<'a> {
        ctx: &'a ViewerContext<'a>,
        query_handle: &'a QueryHandle<'a>,
        schema: &'a [ColumnDescriptor],
        display_record_batches: Option<Vec<DisplayRecordBatch>>,
        query_timeline: Timeline,

        batch_and_row_from_row_nr: BTreeMap<u64, (usize, usize)>,
    }

    impl<'a> egui_table::TableDelegate for MyTableDelegate<'a> {
        fn prefetch_rows(&mut self, mut row_numbers: std::ops::Range<u64>) {
            let start_idx = row_numbers.start.saturating_sub(NUM_HEADER_ROWS);
            let end_idx = row_numbers.end.saturating_sub(NUM_HEADER_ROWS);
            if end_idx <= start_idx {
                return;
            }

            {
                re_tracing::profile_scope!("prefetch_rows");

                dbg!("==========");
                dbg!(start_idx, end_idx);

                let display_record_batches = self
                    .query_handle
                    .get(start_idx, end_idx - start_idx)
                    .into_iter()
                    .map(|record_batch| DisplayRecordBatch::try_new(&record_batch, self.schema))
                    .collect::<Result<Vec<_>, _>>()
                    //TODO: error handling
                    .expect("Failed to create DisplayRecordBatch");

                dbg!(display_record_batches.len());

                let mut offset = start_idx;
                for (batch_idx, batch) in display_record_batches.iter().enumerate() {
                    let batch_len = batch.num_rows() as u64;
                    for row_idx in 0..batch_len {
                        self.batch_and_row_from_row_nr
                            .insert((offset + row_idx) as u64, (batch_idx, row_idx as usize));
                    }
                    offset += batch_len;
                }

                dbg!(&self.batch_and_row_from_row_nr);

                self.display_record_batches = Some(display_record_batches);
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

                    dbg!(row_nr);

                    //TODO: wrong!
                    let latest_at_query = LatestAtQuery::new(self.query_timeline, TimeInt::MAX);
                    let row_id = RowId::ZERO;

                    if let Some(display_record_batches) = &self.display_record_batches {
                        let (batch_nr, batch_index) = self.batch_and_row_from_row_nr[&row_nr]; // Will have been pre-fetched
                        let batch = &display_record_batches[batch_nr];
                        let column = &batch.columns()[cell.col_nr];

                        if ui.is_sizing_pass() {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        } else {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        }
                        column.data_ui(self.ctx, ui, row_id, &latest_at_query, batch_index);
                    } else {
                        panic!("cell_ui called before pre-fetch");
                    }
                });
        }
    }

    let schema = query_handle.schema();
    let num_rows = query_handle.num_rows();

    let mut table_delegate = MyTableDelegate {
        ctx,
        query_handle,
        schema,
        display_record_batches: None,
        query_timeline: query_handle.timeline(),
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
