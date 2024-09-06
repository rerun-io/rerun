use std::collections::BTreeMap;

use re_chunk_store::{ColumnDescriptor, LatestAtQuery, RowId};
use re_dataframe::{LatestAtQueryHandle, RangeQueryHandle, RecordBatch};
use re_log_types::{TimeInt, Timeline};
use re_viewer_context::ViewerContext;

use crate::display_record_batch::DisplayRecordBatch;

/// A query handle for either a latest-at or range query.
pub(crate) enum QueryHandle<'a> {
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

impl<'a> From<LatestAtQueryHandle<'a>> for QueryHandle<'a> {
    fn from(query_handle: LatestAtQueryHandle<'a>) -> Self {
        QueryHandle::LatestAt(query_handle)
    }
}

impl<'a> From<RangeQueryHandle<'a>> for QueryHandle<'a> {
    fn from(query_handle: RangeQueryHandle<'a>) -> Self {
        QueryHandle::Range(query_handle)
    }
}

/// Display the result of a [`QueryHandle`] in a table.
pub(crate) fn dataframe_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_handle: QueryHandle<'_>,
) {
    re_tracing::profile_function!();

    struct MyTableDelegate<'a> {
        ctx: &'a ViewerContext<'a>,
        query_handle: &'a QueryHandle<'a>,
        schema: &'a [ColumnDescriptor],
        display_record_batches: Option<Vec<DisplayRecordBatch>>,
        query_timeline: Timeline,

        batch_and_row_from_row_nr: BTreeMap<u64, (usize, usize)>,
    }

    impl<'a> egui_table::TableDelegate for MyTableDelegate<'a> {
        fn prefetch_columns_and_rows(&mut self, info: &egui_table::PrefetchInfo) {
            re_tracing::profile_function!();

            let start_idx = info.visible_rows.start;
            let end_idx = info.visible_rows.end;

            if end_idx <= start_idx {
                return;
            }

            let display_record_batches = self
                .query_handle
                .get(start_idx, end_idx - start_idx)
                .into_iter()
                .map(|record_batch| DisplayRecordBatch::try_new(&record_batch, self.schema))
                .collect::<Result<Vec<_>, _>>()
                //TODO: error handling
                .expect("Failed to create DisplayRecordBatch");

            let mut offset = start_idx;
            for (batch_idx, batch) in display_record_batches.iter().enumerate() {
                let batch_len = batch.num_rows() as u64;
                for row_idx in 0..batch_len {
                    self.batch_and_row_from_row_nr
                        .insert(offset + row_idx, (batch_idx, row_idx as usize));
                }
                offset += batch_len;
            }

            self.display_record_batches = Some(display_record_batches);
        }

        fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
            egui::Frame::none()
                .inner_margin(egui::Margin::symmetric(4.0, 0.0))
                .show(ui, |ui| {
                    // TODO(emilk): hierarchical row groups
                    ui.strong(self.schema[cell.col_range.start].short_name());
                });
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
                    //TODO: wrong!
                    let latest_at_query = LatestAtQuery::new(self.query_timeline, TimeInt::MAX);
                    let row_id = RowId::ZERO;

                    if let Some(display_record_batches) = &self.display_record_batches {
                        let (batch_nr, batch_index) = self.batch_and_row_from_row_nr[&cell.row_nr]; // Will have been pre-fetched
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
        query_handle: &query_handle,
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
            headers: vec![egui_table::HeaderRow::new(20.0)],
            num_rows,
            row_height: re_ui::DesignTokens::table_line_height(),
            ..Default::default()
        }
        .show(ui, &mut table_delegate);
    });
}
