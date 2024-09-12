use std::collections::BTreeMap;
use std::ops::Range;

use anyhow::Context;
use egui::{NumExt as _, Ui};
use itertools::Itertools;

use re_chunk_store::{ColumnDescriptor, LatestAtQuery, RowId};
use re_dataframe::{LatestAtQueryHandle, RangeQueryHandle, RecordBatch};
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_types_core::Loggable as _;
use re_ui::UiExt as _;
use re_viewer_context::ViewerContext;

use crate::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};
use crate::expanded_rows::{ExpandedRows, ExpandedRowsCache};

/// Display a dataframe table for the provided query.
pub(crate) fn dataframe_ui<'a>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query: impl Into<QueryHandle<'a>>,
    expanded_rows_cache: &mut ExpandedRowsCache,
) {
    dataframe_ui_impl(ctx, ui, &query.into(), expanded_rows_cache);
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

    expanded_rows: ExpandedRows<'a>,

    num_rows: u64,
}

impl DataframeTableDelegate<'_> {
    const LEFT_RIGHT_MARGIN: f32 = 4.0;
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

        //TODO(ab): paint subcell as well
        if cell.row_nr % 2 == 1 {
            // Paint stripes
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
        }

        debug_assert!(cell.row_nr < self.num_rows, "Bug in egui_table");

        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(Self::LEFT_RIGHT_MARGIN, 0.0))
            .show(ui, |ui| {
                self.cell_ui_impl(ui, cell);
            });
    }

    fn row_top_offset(&self, _ctx: &egui::Context, _table_id: egui::Id, row_nr: u64) -> f32 {
        self.expanded_rows.row_top_offset(row_nr)
    }

    fn default_row_height(&self) -> f32 {
        re_ui::DesignTokens::table_line_height()
    }
}

impl DataframeTableDelegate<'_> {
    /// Draw the content of a cell.
    fn cell_ui_impl(&mut self, ui: &mut Ui, cell: &egui_table::CellInfo) {
        re_tracing::profile_function!();

        let display_data = match &self.display_data {
            Ok(display_data) => display_data,
            Err(err) => {
                error_ui(ui, format!("Error with display data: {err}"));
                return;
            }
        };

        let Some(BatchRef {
            batch_idx,
            row_idx: batch_row_idx,
        }) = display_data.batch_ref_from_row.get(&cell.row_nr).copied()
        else {
            error_ui(
                ui,
                "Bug in egui_table: we didn't prefetch what was rendered!",
            );

            return;
        };

        let batch = &display_data.display_record_batches[batch_idx];
        let column = &batch.columns()[cell.col_nr];

        // compute the latest-at query for this row (used to display tooltips)

        // TODO(ab): this is done for every cell but really should be done only once per row
        let timestamp = display_data
            .query_time_column_index
            .and_then(|col_idx| {
                display_data.display_record_batches[batch_idx].columns()[col_idx]
                    .try_decode_time(batch_row_idx)
            })
            .unwrap_or(TimeInt::MAX);
        let latest_at_query = LatestAtQuery::new(self.query_handle.timeline(), timestamp);
        let row_id = display_data
            .row_id_column_index
            .and_then(|col_idx| {
                display_data.display_record_batches[batch_idx].columns()[col_idx]
                    .try_decode_row_id(batch_row_idx)
            })
            .unwrap_or(RowId::ZERO);

        if ui.is_sizing_pass() {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        } else {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        }

        let instance_count = column.instance_count(batch_row_idx);
        let row_expansion = self.expanded_rows.row_expansion(cell.row_nr);

        let instance_indices = std::iter::once(None)
            .chain((0..instance_count).map(Option::Some))
            .take(row_expansion as usize + 1);

        {
            re_tracing::profile_scope!("subcells");

            // how the sub-cell is drawn
            let sub_cell_content =
                |ui: &mut egui::Ui,
                 expanded_rows: &mut ExpandedRows<'_>,
                 sub_cell_index: usize,
                 instance_index: Option<u64>| {
                    // This is called when data actually needs to be drawn (as opposed to summaries like
                    // "N instances" or "N more…").
                    let data_content = |ui: &mut egui::Ui| {
                        column.data_ui(
                            self.ctx,
                            ui,
                            row_id,
                            &latest_at_query,
                            batch_row_idx,
                            instance_index,
                        );
                    };

                    sub_cell_ui(
                        ui,
                        expanded_rows,
                        sub_cell_index,
                        instance_index,
                        instance_count,
                        cell,
                        data_content,
                    );
                };

            split_ui_vertically(
                ui,
                &mut self.expanded_rows,
                instance_indices,
                sub_cell_content,
            );
        }
    }
}

/// Draw a single sub-cell in a table.
///
/// This deals with the row expansion interaction and logic, as well as summarizing the data when
/// necessary. The actual data drawing is delegated to the `data_content` closure.
fn sub_cell_ui(
    ui: &mut egui::Ui,
    expanded_rows: &mut ExpandedRows<'_>,
    sub_cell_index: usize,
    instance_index: Option<u64>,
    instance_count: u64,
    cell: &egui_table::CellInfo,
    data_content: impl Fn(&mut egui::Ui),
) {
    re_tracing::profile_function!();

    let row_expansion = expanded_rows.row_expansion(cell.row_nr);

    /// What kinds of sub-cells might we encounter here?
    enum SubcellKind {
        /// Summary line with content that as zero or one instances, so cannot be expanded.
        Summary,

        /// Summary line with >1 instances, so can be expanded.
        SummaryWithExpand,

        /// A particular instance
        Instance,

        /// There are more instances than available sub-cells, so this is a summary of how many
        /// there are left.
        MoreInstancesSummary(u64 /* remaining instances */),

        /// Not enough instances to fill this sub-cell.
        Blank,
    }

    // The truth table that determines what kind of sub-cell we are dealing with.
    let subcell_kind = match instance_index {
        // First row with >1 instances.
        None if { instance_count > 1 } => SubcellKind::SummaryWithExpand,

        // First row with 0 or 1 instances.
        None => SubcellKind::Summary,

        // Last sub-cell and possibly too many instances to display.
        Some(instance_index)
            if { sub_cell_index as u64 == row_expansion && instance_index < instance_count } =>
        {
            let remaining = instance_count
                .saturating_sub(instance_index)
                .saturating_sub(1);
            if remaining > 0 {
                // +1 is because the "X more…" line takes one instance spot
                SubcellKind::MoreInstancesSummary(remaining + 1)
            } else {
                SubcellKind::Instance
            }
        }

        // Some sub-cell for which an instance exists.
        Some(instance_index) if { instance_index < instance_count } => SubcellKind::Instance,

        // Some sub-cell for which no instance exists.
        Some(_) => SubcellKind::Blank,
    };

    match subcell_kind {
        SubcellKind::Summary => {
            data_content(ui);
        }

        SubcellKind::SummaryWithExpand => {
            let cell_clicked = cell_with_hover_button_ui(ui, &re_ui::icons::EXPAND, |ui| {
                ui.label(format!("{instance_count} instances"));
            });

            if cell_clicked {
                if instance_count == row_expansion {
                    expanded_rows.collapse_row(cell.row_nr);
                } else {
                    expanded_rows.set_row_expansion(cell.row_nr, instance_count);
                }
            }
        }

        SubcellKind::Instance => {
            let cell_clicked = cell_with_hover_button_ui(ui, &re_ui::icons::COLLAPSE, data_content);

            if cell_clicked {
                expanded_rows.collapse_row(cell.row_nr);
            }
        }

        SubcellKind::MoreInstancesSummary(remaining_instances) => {
            let cell_clicked = cell_with_hover_button_ui(ui, &re_ui::icons::EXPAND, |ui| {
                ui.label(format!("{remaining_instances} more…"));
            });

            if cell_clicked {
                expanded_rows.set_row_expansion(cell.row_nr, instance_count);
            }
        }

        SubcellKind::Blank => { /* nothing to show */ }
    }
}

/// Display the result of a [`QueryHandle`] in a table.
fn dataframe_ui_impl(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_handle: &QueryHandle<'_>,
    expanded_rows_cache: &mut ExpandedRowsCache,
) {
    re_tracing::profile_function!();

    let schema = query_handle.schema();

    // The table id mainly drives column widths, so it should be stable across queries leading to
    // the same schema.
    let table_id_salt = egui::Id::new("__dataframe__").with(schema);

    // It's trickier for the row expansion cache.
    //
    // For latest-at view, there is always a single row, so it's ok to validate the cache against
    // the schema. This means that changing the latest-at time stamp does _not_ invalidate, which is
    // desirable. Otherwise, it would be impossible to expand a row when tracking the time panel
    // while it is playing.
    //
    // For range queries, the row layout can change drastically when the query min/max times are
    // modified, so in that case we invalidate against the query expression. This means that the
    // expanded-ness is reset as soon as the min/max boundaries are changed in the selection panel,
    // which is acceptable.
    let row_expansion_id_salt = match query_handle {
        QueryHandle::LatestAt(_) => egui::Id::new("__dataframe_row_exp__").with(schema),
        QueryHandle::Range(query) => egui::Id::new("__dataframe_row_exp__").with(query.query()),
    };

    //egui::Id::new("__dataframe__").with();

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
        expanded_rows: ExpandedRows::new(
            ui.ctx().clone(),
            ui.make_persistent_id(row_expansion_id_salt),
            expanded_rows_cache,
            re_ui::DesignTokens::table_line_height(),
        ),
    };

    let num_sticky_cols = schema
        .iter()
        .take_while(|cd| matches!(cd, ColumnDescriptor::Control(_) | ColumnDescriptor::Time(_)))
        .count();

    egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
        egui_table::Table::new()
            .id_salt(table_id_salt)
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

/// Draw some cell content with an optional, right-aligned, on-hover button.
///
/// Returns true if the button was clicked.
// TODO(ab, emilk): ideally, egui::Sides should work for that, but it doesn't yet support the
// symmetric behavior (left variable width, right fixed width).
fn cell_with_hover_button_ui(
    ui: &mut egui::Ui,
    icon: &'static re_ui::Icon,
    cell_content: impl FnOnce(&mut egui::Ui),
) -> bool {
    if ui.is_sizing_pass() {
        // we don't need space for the icon since it only shows on hover
        cell_content(ui);
        return false;
    }

    let (is_hovering_cell, is_clicked) = ui.input(|i| {
        (
            i.pointer
                .interact_pos()
                .is_some_and(|pos| ui.max_rect().contains(pos)),
            i.pointer.button_clicked(egui::PointerButton::Primary),
        )
    });

    if is_hovering_cell {
        let mut content_rect = ui.max_rect();
        content_rect.max.x = (content_rect.max.x
            - re_ui::DesignTokens::small_icon_size().x
            - re_ui::DesignTokens::text_to_icon_padding())
        .at_least(content_rect.min.x);

        let button_rect = egui::Rect::from_x_y_ranges(
            (content_rect.max.x + re_ui::DesignTokens::text_to_icon_padding())
                ..=ui.max_rect().max.x,
            ui.max_rect().y_range(),
        );

        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
        cell_content(&mut content_ui);

        let mut button_ui = ui.new_child(egui::UiBuilder::new().max_rect(button_rect));
        button_ui.visuals_mut().widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
        button_ui.visuals_mut().widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;
        button_ui.add(egui::Button::image(
            icon.as_image()
                .fit_to_exact_size(re_ui::DesignTokens::small_icon_size())
                .tint(button_ui.visuals().widgets.noninteractive.text_color()),
        ));

        is_clicked
    } else {
        cell_content(ui);
        false
    }
}

/// Helper to draw individual rows (aka sub-cell) into an expanded cell in a table.
///
/// `context`: whatever mutable context is necessary for the `sub_cell_content`
/// `sub_cell_data`: the data to be displayed in each sub-cell
/// `sub_cell_content`: the function to draw the content of each sub-cell
fn split_ui_vertically<I, Ctx>(
    ui: &mut egui::Ui,
    context: &mut Ctx,
    sub_cell_data: impl Iterator<Item = I>,
    sub_cell_content: impl Fn(&mut egui::Ui, &mut Ctx, usize, I),
) {
    re_tracing::profile_function!();

    // Empirical testing shows that iterating over all instances can take multiple tens of ms
    // when the instance count is very large (which is common). So we use the clip rectangle to
    // determine exactly which instances are visible and iterate only over those.
    let visible_y_range = ui.clip_rect().y_range();
    let total_y_range = ui.max_rect().y_range();

    let start_row = ((visible_y_range.min - total_y_range.min)
        / re_ui::DesignTokens::table_line_height())
    .at_least(0.0)
    .floor() as usize;

    let end_row = ((visible_y_range.max - total_y_range.min)
        / re_ui::DesignTokens::table_line_height())
    .at_least(0.0)
    .ceil() as usize;

    for (sub_cell_index, item_data) in sub_cell_data
        .enumerate()
        .skip(start_row)
        .take(end_row.saturating_sub(start_row))
    {
        let sub_cell_rect = egui::Rect::from_min_size(
            ui.cursor().min
                + egui::vec2(
                    0.0,
                    sub_cell_index as f32 * re_ui::DesignTokens::table_line_height(),
                ),
            egui::vec2(
                ui.available_width(),
                re_ui::DesignTokens::table_line_height(),
            ),
        );

        // During animation, there may be more sub-cells than can possibly fit. If so, no point in
        // continuing to draw them.
        if !ui.max_rect().intersects(sub_cell_rect) {
            return;
        }

        let mut sub_cell_ui = ui.new_child(egui::UiBuilder::new().max_rect(sub_cell_rect));

        sub_cell_content(&mut sub_cell_ui, context, sub_cell_index, item_data);
    }
}
