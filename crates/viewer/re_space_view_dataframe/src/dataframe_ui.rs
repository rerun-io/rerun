use std::collections::BTreeMap;
use std::ops::Range;

use anyhow::Context;
use egui::NumExt as _;
use itertools::Itertools;

use re_chunk_store::{ColumnDescriptor, LatestAtQuery, RowId};
use re_dataframe::{LatestAtQueryHandle, RangeQueryHandle, RecordBatch};
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_types_core::Loggable as _;
use re_ui::UiExt as _;
use re_viewer_context::ViewerContext;

use crate::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};

/// Display a dataframe table for the provided query.
pub(crate) fn dataframe_ui<'a>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query: impl Into<QueryHandle<'a>>,
) {
    dataframe_ui_impl(ctx, ui, &query.into());
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
                // latest-at queries only have one row
                debug_assert_eq!((start, num_rows), (0, 1));

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

#[derive(Clone, Copy)]
struct BatchRef {
    /// Which batch?
    batch_idx: usize,

    /// Which row within the batch?
    row_idx: usize,
}

/// This structure maintains the data for displaying rows in a table.
///
/// Row data is stored in a bunch of [`DisplayRecordBatch`], which are created from
/// [`RecordBatch`]s. We also maintain a mapping for each row number to the corresponding record
/// batch and the index inside it.
struct RowsDisplayData {
    /// The [`DisplayRecordBatch`]s to display.
    display_record_batches: Vec<DisplayRecordBatch>,

    /// For each row to be displayed, where can we find the data?
    batch_ref_from_row: BTreeMap<u64, BatchRef>,

    /// The index of the time column corresponding to the query timeline.
    query_time_column_index: Option<usize>,

    /// The index of the time column corresponding the row IDs.
    row_id_column_index: Option<usize>,
}

impl RowsDisplayData {
    fn try_new(
        row_indices: &Range<u64>,
        record_batches: Vec<RecordBatch>,
        schema: &[ColumnDescriptor],
        query_timeline: &Timeline,
    ) -> Result<Self, DisplayRecordBatchError> {
        let display_record_batches = record_batches
            .into_iter()
            .map(|record_batch| DisplayRecordBatch::try_new(&record_batch, schema))
            .collect::<Result<Vec<_>, _>>()?;

        let mut batch_ref_from_row = BTreeMap::new();
        let mut offset = row_indices.start;
        for (batch_idx, batch) in display_record_batches.iter().enumerate() {
            let batch_len = batch.num_rows();
            for row_idx in 0..batch_len {
                batch_ref_from_row.insert(offset + row_idx as u64, BatchRef { batch_idx, row_idx });
            }
            offset += batch_len as u64;
        }

        // find the time column
        let query_time_column_index = schema
            .iter()
            .find_position(|desc| match desc {
                ColumnDescriptor::Time(time_column_desc) => {
                    &time_column_desc.timeline == query_timeline
                }
                _ => false,
            })
            .map(|(pos, _)| pos);

        // find the row id column
        let row_id_column_index = schema
            .iter()
            .find_position(|desc| match desc {
                ColumnDescriptor::Control(control_column_desc) => {
                    control_column_desc.component_name == RowId::name()
                }
                _ => false,
            })
            .map(|(pos, _)| pos);

        Ok(Self {
            display_record_batches,
            batch_ref_from_row,
            query_time_column_index,
            row_id_column_index,
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

    num_rows: u64,
}

impl DataframeTableDelegate<'_> {
    const LEFT_RIGHT_MARGIN: f32 = 4.0;
}

impl<'a> egui_table::TableDelegate for DataframeTableDelegate<'a> {
    fn default_row_height(&self) -> f32 {
        re_ui::DesignTokens::table_line_height()
    }

    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        re_tracing::profile_function!();

        let data = RowsDisplayData::try_new(
            &info.visible_rows,
            self.query_handle.get(
                info.visible_rows.start,
                info.visible_rows.end - info.visible_rows.start,
            ),
            self.schema,
            &self.query_handle.timeline(),
        );

        self.display_data = data.context("Failed to create display data");
    }

    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(4.0, 0.0))
            .show(ui, |ui| {
                if cell.row_nr == 0 {
                    if let Some(entity_path) = &self.header_entity_paths[cell.group_index] {
                        //TODO(ab): factor this into a helper as soon as we use it elsewhere
                        let text = entity_path.to_string();
                        let font_id = egui::TextStyle::Body.resolve(ui.style());
                        let text_color = ui.visuals().text_color();
                        let galley = ui
                            .painter()
                            .layout(text, font_id, text_color, f32::INFINITY);

                        // Put the text leftmost in the clip rect (so it is always visible)
                        let mut pos = egui::Align2::LEFT_CENTER
                            .anchor_size(
                                ui.clip_rect().shrink(Self::LEFT_RIGHT_MARGIN).left_center(),
                                galley.size(),
                            )
                            .min;

                        // … but not so far to the right that it doesn't fit.
                        pos.x = pos.x.at_most(ui.max_rect().right() - galley.size().x);

                        ui.put(
                            egui::Rect::from_min_size(pos, galley.size()),
                            egui::Label::new(galley),
                        );
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

        debug_assert!(cell.row_nr < self.num_rows, "Bug in egui_table");

        let display_data = match &self.display_data {
            Ok(display_data) => display_data,
            Err(err) => {
                error_ui(ui, format!("Error with display data: {err}"));
                return;
            }
        };

        let cell_ui = |ui: &mut egui::Ui| {
            if let Some(BatchRef { batch_idx, row_idx }) =
                display_data.batch_ref_from_row.get(&cell.row_nr).copied()
            {
                let batch = &display_data.display_record_batches[batch_idx];
                let column = &batch.columns()[cell.col_nr];

                // compute the latest-at query for this row (used to display tooltips)
                let timestamp = display_data
                    .query_time_column_index
                    .and_then(|col_idx| {
                        display_data.display_record_batches[batch_idx].columns()[col_idx]
                            .try_decode_time(row_idx)
                    })
                    .unwrap_or(TimeInt::MAX);
                let latest_at_query = LatestAtQuery::new(self.query_handle.timeline(), timestamp);
                let row_id = display_data
                    .row_id_column_index
                    .and_then(|col_idx| {
                        display_data.display_record_batches[batch_idx].columns()[col_idx]
                            .try_decode_row_id(row_idx)
                    })
                    .unwrap_or(RowId::ZERO);

                if ui.is_sizing_pass() {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                } else {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                }
                column.data_ui(self.ctx, ui, row_id, &latest_at_query, row_idx);
            } else {
                error_ui(
                    ui,
                    "Bug in egui_table: we didn't prefetch what was rendered!",
                );
            }
        };

        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(Self::LEFT_RIGHT_MARGIN, 0.0))
            .show(ui, cell_ui);
    }
}

/// Display the result of a [`QueryHandle`] in a table.
fn dataframe_ui_impl(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, query_handle: &QueryHandle<'_>) {
    re_tracing::profile_function!();

    let schema = query_handle.schema();
    let (header_groups, header_entity_paths) = column_groups_for_entity(schema);

    let num_rows = query_handle.num_rows();

    let mut table_delegate = DataframeTableDelegate {
        ctx,
        query_handle,
        schema,
        header_entity_paths,
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
                    height: re_ui::DesignTokens::table_header_height(),
                    groups: header_groups,
                },
                egui_table::HeaderRow::new(re_ui::DesignTokens::table_header_height()),
            ])
            .num_rows(num_rows)
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
        #[allow(clippy::single_range_in_vec_init)]
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
