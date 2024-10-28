use std::collections::BTreeMap;
use std::ops::Range;

use anyhow::Context;
use egui::NumExt as _;
use itertools::Itertools;

use re_chunk_store::external::re_chunk::ArrowArray;
use re_chunk_store::{ColumnDescriptor, LatestAtQuery};
use re_dataframe::QueryHandle;
use re_log_types::{EntityPath, TimeInt, Timeline, TimelineName};
use re_types_core::ComponentName;
use re_ui::UiExt as _;
use re_viewer_context::{SpaceViewId, SystemCommandSender, ViewerContext};

use crate::display_record_batch::{DisplayRecordBatch, DisplayRecordBatchError};
use crate::expanded_rows::{ExpandedRows, ExpandedRowsCache};

/// Ui actions triggered by the dataframe UI to be handled by the calling code.
pub(crate) enum HideColumnAction {
    HideTimeColumn {
        timeline_name: TimelineName,
    },

    HideComponentColumn {
        entity_path: EntityPath,
        component_name: ComponentName,
    },
}

/// Display a dataframe table for the provided query.
pub(crate) fn dataframe_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_handle: &re_dataframe::QueryHandle<'_>,
    expanded_rows_cache: &mut ExpandedRowsCache,
    space_view_id: &SpaceViewId,
) -> Vec<HideColumnAction> {
    re_tracing::profile_function!();

    let selected_columns = query_handle
        .selected_contents()
        .iter()
        .map(|(_, desc)| desc.clone())
        .collect::<Vec<_>>();

    // The table id mainly drives column widths, along with the id of each column. Empirically, the
    // user experience is better if we have stable column width even when the query changes (which
    // can, in turn, change the column's content).
    let table_id_salt = egui::Id::new("__dataframe__").with(space_view_id);

    // For the row expansion cache, we invalidate more aggressively for now, because the expanded
    // state is stored against a row index (not unique id like columns). This means rows will more
    // often auto-collapse when the query is modified.
    let row_expansion_id_salt = egui::Id::new("__dataframe_row_exp__")
        .with(space_view_id)
        .with(&selected_columns)
        .with(query_handle.query());

    let (header_groups, header_entity_paths) = column_groups_for_entity(&selected_columns);

    let num_rows = query_handle.num_rows();

    let mut table_delegate = DataframeTableDelegate {
        ctx,
        query_handle,
        selected_columns: &selected_columns,
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
        hide_column_actions: vec![],
    };

    let num_sticky_cols = selected_columns
        .iter()
        .take_while(|cd| matches!(cd, ColumnDescriptor::Time(_)))
        .count();

    egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
        egui_table::Table::new()
            .id_salt(table_id_salt)
            .columns(
                selected_columns
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

    table_delegate.hide_column_actions
}

#[derive(Debug, Clone, Copy)]
struct BatchRef {
    /// Which batch?
    batch_idx: usize,

    /// Which row within the batch?
    row_idx: usize,
}

/// This structure maintains the data for displaying rows in a table.
///
/// Row data is stored in a bunch of [`DisplayRecordBatch`], which are created from the rows
/// returned by the query. We also maintain a mapping for each row number to the corresponding
/// display record batch and the index inside it.
#[derive(Debug)]
struct RowsDisplayData {
    /// The [`DisplayRecordBatch`]s to display.
    display_record_batches: Vec<DisplayRecordBatch>,

    /// For each row to be displayed, where can we find the data?
    batch_ref_from_row: BTreeMap<u64, BatchRef>,

    /// The index of the time column corresponding to the query timeline.
    query_time_column_index: Option<usize>,
}

impl RowsDisplayData {
    fn try_new(
        row_indices: &Range<u64>,
        row_data: Vec<Vec<Box<dyn ArrowArray>>>,
        selected_columns: &[ColumnDescriptor],
        query_timeline: &Timeline,
    ) -> Result<Self, DisplayRecordBatchError> {
        let display_record_batches = row_data
            .into_iter()
            .map(|data| DisplayRecordBatch::try_new(&data, selected_columns))
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
        let query_time_column_index = selected_columns
            .iter()
            .find_position(|desc| match desc {
                ColumnDescriptor::Time(time_column_desc) => {
                    &time_column_desc.timeline == query_timeline
                }
                ColumnDescriptor::Component(_) => false,
            })
            .map(|(pos, _)| pos);

        Ok(Self {
            display_record_batches,
            batch_ref_from_row,
            query_time_column_index,
        })
    }
}

/// [`egui_table::TableDelegate`] implementation for displaying a [`QueryHandle`] in a table.
struct DataframeTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    query_handle: &'a QueryHandle<'a>,
    selected_columns: &'a [ColumnDescriptor],
    header_entity_paths: Vec<Option<EntityPath>>,
    display_data: anyhow::Result<RowsDisplayData>,

    expanded_rows: ExpandedRows<'a>,

    num_rows: u64,
    hide_column_actions: Vec<HideColumnAction>,
}

impl DataframeTableDelegate<'_> {
    const LEFT_RIGHT_MARGIN: f32 = 4.0;
}

impl<'a> egui_table::TableDelegate for DataframeTableDelegate<'a> {
    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        re_tracing::profile_function!();

        // TODO(ab): actual static-only support
        let filtered_index = self.query_handle.query().filtered_index.unwrap_or_default();

        self.query_handle
            .seek_to_row(info.visible_rows.start as usize);
        let data = std::iter::from_fn(|| self.query_handle.next_row())
            .take((info.visible_rows.end - info.visible_rows.start) as usize)
            .collect();

        let data = RowsDisplayData::try_new(
            &info.visible_rows,
            data,
            self.selected_columns,
            &filtered_index,
        );

        self.display_data = data.context("Failed to create display data");
    }

    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        if ui.is_sizing_pass() {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        } else {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        }

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

                        // Extra padding for this being a button.
                        let size = galley.size() + 2.0 * ui.spacing().button_padding;

                        // Put the text leftmost in the clip rect (so it is always visible)
                        let mut pos = egui::Align2::LEFT_CENTER
                            .anchor_size(
                                ui.clip_rect().shrink(Self::LEFT_RIGHT_MARGIN).left_center(),
                                size,
                            )
                            .min;

                        // … but not so far to the right that it doesn't fit.
                        pos.x = pos.x.at_most(ui.max_rect().right() - size.x);

                        let item = re_viewer_context::Item::from(entity_path.clone());
                        let is_selected = self.ctx.selection().contains_item(&item);
                        let response = ui.put(
                            egui::Rect::from_min_size(pos, size),
                            egui::SelectableLabel::new(is_selected, galley),
                        );
                        self.ctx.select_hovered_on_click(&response, item);

                        // TODO(emilk): expand column(s) to make sure the text fits (requires egui_table fix).
                    }
                } else if cell.row_nr == 1 {
                    let column = &self.selected_columns[cell.col_range.start];

                    // TODO(ab): actual static-only support
                    let filtered_index =
                        self.query_handle.query().filtered_index.unwrap_or_default();

                    // if this column can actually be hidden, then that's the corresponding action
                    let hide_action = match column {
                        ColumnDescriptor::Time(desc) => {
                            (desc.timeline != filtered_index).then(|| {
                                HideColumnAction::HideTimeColumn {
                                    timeline_name: *desc.timeline.name(),
                                }
                            })
                        }

                        ColumnDescriptor::Component(desc) => {
                            Some(HideColumnAction::HideComponentColumn {
                                entity_path: desc.entity_path.clone(),
                                component_name: desc.component_name,
                            })
                        }
                    };

                    let header_ui = |ui: &mut egui::Ui| {
                        let text = egui::RichText::new(column.short_name()).strong();

                        let is_selected = match column {
                            ColumnDescriptor::Time(descr) => {
                                &descr.timeline == self.ctx.rec_cfg.time_ctrl.read().timeline()
                            }
                            ColumnDescriptor::Component(component_column_descriptor) => self
                                .ctx
                                .selection()
                                .contains_item(&re_viewer_context::Item::ComponentPath(
                                    component_column_descriptor.component_path(),
                                )),
                        };

                        let response = ui.selectable_label(is_selected, text);

                        match column {
                            ColumnDescriptor::Time(descr) => {
                                if response.clicked() {
                                    self.ctx.command_sender.send_system(
                                        re_viewer_context::SystemCommand::SetActiveTimeline {
                                            rec_id: self.ctx.recording_id().clone(),
                                            timeline: descr.timeline,
                                        },
                                    );
                                }
                            }
                            ColumnDescriptor::Component(component_column_descriptor) => {
                                self.ctx.select_hovered_on_click(
                                    &response,
                                    re_viewer_context::Item::ComponentPath(
                                        component_column_descriptor.component_path(),
                                    ),
                                );
                            }
                        }
                    };

                    if let Some(hide_action) = hide_action {
                        let hide_clicked = cell_with_hover_button_ui(
                            ui,
                            &re_ui::icons::VISIBLE,
                            CellStyle::Header,
                            header_ui,
                        );

                        if hide_clicked {
                            self.hide_column_actions.push(hide_action);
                        }
                    } else {
                        header_ui(ui);
                    }
                } else {
                    // this should never happen
                    error_ui(ui, format!("Unexpected header row_nr: {}", cell.row_nr));
                }
            });
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        re_tracing::profile_function!();

        debug_assert!(cell.row_nr < self.num_rows, "Bug in egui_table");

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

        // TODO(ab): actual static-only support
        let filtered_index = self.query_handle.query().filtered_index.unwrap_or_default();
        let latest_at_query = LatestAtQuery::new(filtered_index, timestamp);

        if ui.is_sizing_pass() {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        } else {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        }

        let instance_count = column.instance_count(batch_row_idx);
        let additional_lines = self.expanded_rows.additional_lines_for_row(cell.row_nr);

        let is_row_odd = self.expanded_rows.is_row_odd(cell.row_nr);

        // Iterate over the top line (the summary, thus the `None`), and all additional lines.
        // Note: we must iterate over all lines regardless of the actual number of instances so that
        // the zebra stripes are properly drawn.
        let instance_indices = std::iter::once(None).chain((0..additional_lines).map(Option::Some));

        {
            re_tracing::profile_scope!("lines");

            // how the line is drawn
            let line_content = |ui: &mut egui::Ui,
                                expanded_rows: &mut ExpandedRows<'_>,
                                line_index: usize,
                                instance_index: Option<u64>| {
                // Draw the alternating background color.
                let is_line_odd = is_row_odd ^ (line_index % 2 == 1);
                if is_line_odd {
                    ui.painter()
                        .rect_filled(ui.max_rect(), 0.0, ui.visuals().faint_bg_color);
                }

                // This is called when data actually needs to be drawn (as opposed to summaries like
                // "N instances" or "N more…").
                let data_content = |ui: &mut egui::Ui| {
                    column.data_ui(
                        self.ctx,
                        ui,
                        &latest_at_query,
                        batch_row_idx,
                        instance_index,
                    );
                };

                // Draw the cell content with some margin.
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(Self::LEFT_RIGHT_MARGIN, 0.0))
                    .show(ui, |ui| {
                        line_ui(
                            ui,
                            expanded_rows,
                            line_index,
                            instance_index,
                            instance_count,
                            cell,
                            data_content,
                        );
                    });
            };

            split_ui_vertically(ui, &mut self.expanded_rows, instance_indices, line_content);
        }
    }

    fn row_top_offset(&self, _ctx: &egui::Context, _table_id: egui::Id, row_nr: u64) -> f32 {
        self.expanded_rows.row_top_offset(row_nr)
    }

    fn default_row_height(&self) -> f32 {
        re_ui::DesignTokens::table_line_height()
    }
}

/// Draw a single line in a table.
///
/// This deals with the row expansion interaction and logic, as well as summarizing the data when
/// necessary. The actual data drawing is delegated to the `data_content` closure.
fn line_ui(
    ui: &mut egui::Ui,
    expanded_rows: &mut ExpandedRows<'_>,
    line_index: usize,
    instance_index: Option<u64>,
    instance_count: u64,
    cell: &egui_table::CellInfo,
    data_content: impl Fn(&mut egui::Ui),
) {
    re_tracing::profile_function!();

    let row_expansion = expanded_rows.additional_lines_for_row(cell.row_nr);

    /// What kinds of lines might we encounter here?
    enum SubcellKind {
        /// Summary line with content that as zero or one instances, so cannot be expanded.
        Summary,

        /// Summary line with >1 instances, so can be expanded.
        SummaryWithExpand,

        /// A particular instance
        Instance,

        /// There are more instances than available lines, so this is a summary of how many
        /// there are left.
        MoreInstancesSummary { remaining_instances: u64 },

        /// Not enough instances to fill this line.
        Blank,
    }

    // The truth table that determines what kind of line we are dealing with.
    let subcell_kind = match instance_index {
        // First row with >1 instances.
        None if { instance_count > 1 } => SubcellKind::SummaryWithExpand,

        // First row with 0 or 1 instances.
        None => SubcellKind::Summary,

        // Last line and possibly too many instances to display.
        Some(instance_index)
            if { line_index as u64 == row_expansion && instance_index < instance_count } =>
        {
            let remaining = instance_count
                .saturating_sub(instance_index)
                .saturating_sub(1);
            if remaining > 0 {
                // +1 is because the "X more…" line takes one instance spot
                SubcellKind::MoreInstancesSummary {
                    remaining_instances: remaining + 1,
                }
            } else {
                SubcellKind::Instance
            }
        }

        // Some line for which an instance exists.
        Some(instance_index) if { instance_index < instance_count } => SubcellKind::Instance,

        // Some line for which no instance exists.
        Some(_) => SubcellKind::Blank,
    };

    match subcell_kind {
        SubcellKind::Summary => {
            data_content(ui);
        }

        SubcellKind::SummaryWithExpand => {
            let cell_clicked = cell_with_hover_button_ui(
                ui,
                &re_ui::icons::EXPAND,
                CellStyle::InstanceData,
                |ui| {
                    ui.label(format!(
                        "{} instances",
                        re_format::format_uint(instance_count)
                    ));
                },
            );

            if cell_clicked {
                if instance_count == row_expansion {
                    expanded_rows.remove_additional_lines_for_row(cell.row_nr);
                } else {
                    expanded_rows.set_additional_lines_for_row(cell.row_nr, instance_count);
                }
            }
        }

        SubcellKind::Instance => {
            let cell_clicked = cell_with_hover_button_ui(
                ui,
                &re_ui::icons::COLLAPSE,
                CellStyle::InstanceData,
                data_content,
            );

            if cell_clicked {
                expanded_rows.remove_additional_lines_for_row(cell.row_nr);
            }
        }

        SubcellKind::MoreInstancesSummary {
            remaining_instances,
        } => {
            let cell_clicked = cell_with_hover_button_ui(
                ui,
                &re_ui::icons::EXPAND,
                CellStyle::InstanceData,
                |ui| {
                    ui.label(format!(
                        "{} more…",
                        re_format::format_uint(remaining_instances)
                    ));
                },
            );

            if cell_clicked {
                expanded_rows.set_additional_lines_for_row(cell.row_nr, instance_count);
            }
        }

        SubcellKind::Blank => { /* nothing to show */ }
    }
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

/// Style for [`cell_with_hover_button_ui`].
#[derive(Debug, Clone, Copy)]
enum CellStyle {
    /// Icon is brighter but must be directly clicked.
    Header,

    /// Icon is dimmer but can be clicked from anywhere in the cell.
    InstanceData,
}

/// Draw some cell content with a right-aligned, on-hover button.
///
/// The button is only displayed when the cell is hovered. Returns true if the button was clicked.
/// Both the visuals and the click behavior is affected by the `style`.
// TODO(ab, emilk): ideally, egui::Sides should work for that, but it doesn't yet support the
// asymmetric behavior (left variable width, right fixed width).
// See https://github.com/emilk/egui/issues/5116
fn cell_with_hover_button_ui(
    ui: &mut egui::Ui,
    icon: &'static re_ui::Icon,
    style: CellStyle,
    cell_content: impl FnOnce(&mut egui::Ui),
) -> bool {
    if ui.is_sizing_pass() {
        // we don't need space for the icon since it only shows on hover
        cell_content(ui);
        return false;
    }

    let is_hovering_cell = ui.rect_contains_pointer(ui.max_rect());

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

        let button_tint = match style {
            CellStyle::Header => ui.visuals().widgets.active.text_color(),
            CellStyle::InstanceData => ui.visuals().widgets.noninteractive.text_color(),
        };

        let mut button_ui = ui.new_child(egui::UiBuilder::new().max_rect(button_rect));
        button_ui.visuals_mut().widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
        button_ui.visuals_mut().widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;
        button_ui.add(egui::Button::image(
            icon.as_image()
                .fit_to_exact_size(re_ui::DesignTokens::small_icon_size())
                .tint(button_tint),
        ));

        let click_happened = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));

        // was this click relevant?
        match style {
            CellStyle::Header => {
                click_happened && button_ui.rect_contains_pointer(button_ui.max_rect())
            }
            CellStyle::InstanceData => click_happened,
        }
    } else {
        cell_content(ui);
        false
    }
}

/// Helper to draw individual lines into an expanded cell in a table.
///
/// `context`: whatever mutable context is necessary for the `line_content_ui`
/// `line_data`: the data to be displayed in each line
/// `line_content_ui`: the function to draw the content of each line
fn split_ui_vertically<Item, Ctx>(
    ui: &mut egui::Ui,
    context: &mut Ctx,
    line_data: impl Iterator<Item = Item>,
    line_content_ui: impl Fn(&mut egui::Ui, &mut Ctx, usize, Item),
) {
    re_tracing::profile_function!();

    // Empirical testing shows that iterating over all instances can take multiple tens of ms
    // when the instance count is very large (which is common). So we use the clip rectangle to
    // determine exactly which instances are visible and iterate only over those.
    let visible_y_range = ui.clip_rect().y_range();
    let total_y_range = ui.max_rect().y_range();

    let line_height = re_ui::DesignTokens::table_line_height();

    // Note: converting float to unsigned ints implicitly saturate negative values to 0
    let start_row = ((visible_y_range.min - total_y_range.min) / line_height).floor() as usize;

    let end_row = ((visible_y_range.max - total_y_range.min) / line_height).ceil() as usize;

    let ui_left_top = ui.cursor().min;
    let row_size = egui::vec2(ui.available_width(), line_height);

    for (line_index, item_data) in line_data
        .enumerate()
        .skip(start_row)
        .take(end_row.saturating_sub(start_row))
    {
        let line_rect = egui::Rect::from_min_size(
            ui_left_top + egui::Vec2::DOWN * (line_index as f32 * line_height),
            row_size,
        );

        // During animation, there may be more lines than can possibly fit. If so, no point in
        // continuing to draw them.
        if !ui.max_rect().intersects(line_rect) {
            return;
        }

        ui.scope_builder(egui::UiBuilder::new().max_rect(line_rect), |ui| {
            line_content_ui(ui, context, line_index, item_data);
        });
    }
}
