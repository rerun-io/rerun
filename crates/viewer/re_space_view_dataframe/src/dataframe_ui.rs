use std::collections::BTreeMap;
use std::ops::Range;

use anyhow::Context;

use re_chunk_store::{ColumnDescriptor, LatestAtQuery, RowId};
use re_dataframe::{LatestAtQueryHandle, RangeQueryHandle, RecordBatch};
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_ui::UiExt as _;
use re_viewer_context::ViewerContext;

use crate::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};

pub(crate) fn dataframe_ui<'a>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query: impl Into<QueryHandle<'a>>,
) {
    dataframe_ui_impl(ctx, ui, query.into());
}

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

/// This structure maintains the data for displaying rows in a table.
///
/// Row data is stored in a bunch of [`DisplayRecordBatch`], which are created from
/// [`RecordBatch`]s. We also maintain a mapping for each row number to the corresponding record
/// batch and the inedex inside it.
struct RowsDisplayData {
    //row_ids: Vec<RowId>,
    display_record_batches: Vec<DisplayRecordBatch>,
    batch_and_row_from_row_nr: BTreeMap<u64, (usize, usize)>,
}

impl RowsDisplayData {
    fn try_new(
        row_indices: &Range<u64>,
        record_batches: Vec<RecordBatch>,
        schema: &[ColumnDescriptor],
    ) -> Result<Self, DisplayRecordBatchError> {
        let display_record_batches = record_batches
            .into_iter()
            .map(|record_batch| DisplayRecordBatch::try_new(&record_batch, schema))
            .collect::<Result<Vec<_>, _>>()?;

        let mut batch_and_row_from_row_nr = BTreeMap::new();
        let mut offset = row_indices.start;
        for (batch_idx, batch) in display_record_batches.iter().enumerate() {
            let batch_len = batch.num_rows() as u64;
            for row_idx in 0..batch_len {
                batch_and_row_from_row_nr.insert(offset + row_idx, (batch_idx, row_idx as usize));
            }
            offset += batch_len;
        }

        Ok(Self {
            //row_ids: row_indices.collect(),
            display_record_batches,
            batch_and_row_from_row_nr,
        })
    }
}

/// [`egui_table::TableDelegate`] implementation for displaying a [`QueryHandle`] in a table.
struct DataframeTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    query_handle: &'a QueryHandle<'a>,
    schema: &'a [ColumnDescriptor],
    header_entity_paths: Vec<Option<EntityPath>>,
    display_data: anyhow::Result<RowsDisplayData>,
    //display_record_batches: Option<Vec<DisplayRecordBatch>>,
    query_timeline: Timeline,

    num_rows: u64,
    //batch_and_row_from_row_nr: BTreeMap<u64, (usize, usize)>,
}

impl<'a> egui_table::TableDelegate for DataframeTableDelegate<'a> {
    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        re_tracing::profile_function!();

        let data = RowsDisplayData::try_new(
            &info.visible_rows,
            self.query_handle.get(
                info.visible_rows.start,
                info.visible_rows.end - info.visible_rows.start,
            ),
            self.schema,
        );

        self.display_data = data.with_context(|| "Failed to create display data");
    }

    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(4.0, 0.0))
            .show(ui, |ui| {
                if cell.row_nr == 0 {
                    if let Some(entity_path) = &self.header_entity_paths[cell.group_index] {
                        ui.label(entity_path.to_string());
                    }
                } else if cell.row_nr == 1 {
                    ui.strong(self.schema[cell.col_range.start].short_name());
                } else {
                    // this should never happen
                    error_ui(ui, format!("Unexpected header row_nr: {}", cell.row_nr));
                }
            });
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        re_tracing::profile_function!();

        if cell.row_nr % 2 == 1 {
            // Paint stripes
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
        }

        // sanity check, this should never happen
        if cell.row_nr >= self.num_rows {
            error_ui(
                ui,
                format!(
                    "Unexpected row_nr: {} (table row count {})",
                    cell.row_nr, self.num_rows
                ),
            );
            return;
        }

        let display_data = match &self.display_data {
            Ok(display_data) => display_data,
            Err(err) => {
                error_ui(ui, format!("Error with display data: {err}"));
                return;
            }
        };

        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(4.0, 0.0))
            .show(ui, |ui| {
                //TODO: wrong, we must pass the actual timestamp of the row
                let latest_at_query = LatestAtQuery::new(self.query_timeline, TimeInt::MAX);
                //TODO: wrong, we must pass the actual row_id (if we have it)
                let row_id = RowId::ZERO;

                if let Some((batch_nr, batch_index)) = display_data
                    .batch_and_row_from_row_nr
                    .get(&cell.row_nr)
                    .copied()
                {
                    let batch = &display_data.display_record_batches[batch_nr];
                    let column = &batch.columns()[cell.col_nr];

                    if ui.is_sizing_pass() {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    } else {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    }
                    column.data_ui(self.ctx, ui, row_id, &latest_at_query, batch_index);
                } else {
                    error_ui(
                        ui,
                        "Bug in egui_table: we didn't prefetch what was rendered!",
                    );
                }
            });
    }
}

/// Display the result of a [`QueryHandle`] in a table.
fn dataframe_ui_impl(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, query_handle: QueryHandle<'_>) {
    re_tracing::profile_function!();

    let schema = query_handle.schema();
    let (header_groups, header_entity_paths) = column_groups_for_entity(schema);

    let num_rows = query_handle.num_rows();

    let mut table_delegate = DataframeTableDelegate {
        ctx,
        query_handle: &query_handle,
        schema,
        header_entity_paths,
        query_timeline: query_handle.timeline(),
        num_rows,
        display_data: Err(anyhow::anyhow!(
            "No row data, `fetch_columns_and_rows` not called."
        )),
    };

    let num_sticky_cols = schema
        .iter()
        .take_while(|cd| matches!(cd, ColumnDescriptor::Control(_) | ColumnDescriptor::Time(_)))
        .count();

    egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
        egui_table::Table::new()
            .columns(
                schema
                    .iter()
                    .map(|column_descr| {
                        egui_table::Column::new(200.0)
                            .resizable(true)
                            .id(egui::Id::new(column_descr))
                    })
                    .collect::<Vec<_>>(),
            )
            .num_sticky_cols(num_sticky_cols)
            .headers(vec![
                egui_table::HeaderRow {
                    height: 20.0,
                    groups: header_groups,
                },
                egui_table::HeaderRow::new(20.0),
            ])
            .num_rows(num_rows)
            .row_height(re_ui::DesignTokens::table_line_height())
            .show(ui, &mut table_delegate);
    });
}

/// Groups column by entity paths.
fn column_groups_for_entity(
    columns: &[ColumnDescriptor],
) -> (Vec<Range<usize>>, Vec<Option<EntityPath>>) {
    if columns.is_empty() {
        (vec![], vec![])
    } else if columns.len() == 1 {
        (vec![0..1], vec![columns[0].entity_path().cloned()])
    } else {
        let mut groups = vec![];
        let mut entity_paths = vec![];
        let mut start = 0;
        let mut current_entity = columns[0].entity_path();
        for (i, column) in columns.iter().enumerate().skip(1) {
            if column.entity_path() != current_entity {
                groups.push(start..i);
                entity_paths.push(current_entity.cloned());
                start = i;
                current_entity = column.entity_path();
            }
        }
        groups.push(start..columns.len());
        entity_paths.push(current_entity.cloned());
        (groups, entity_paths)
    }
}

fn error_ui(ui: &mut egui::Ui, error: impl AsRef<str>) {
    let error = error.as_ref();
    ui.error_label(error);
    re_log::warn_once!("{error}");
}
