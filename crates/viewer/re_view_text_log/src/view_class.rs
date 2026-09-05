use std::collections::BTreeSet;

use egui::emath::GuiRounding as _;
use re_chunk_store::TimeInt;
use re_data_ui::item_ui::{self, timeline_button};
use re_dataframe_ui::apply_table_style_fixes;
use re_log::ResultExt as _;
use re_log_types::{EntityPath, TimelineName};
use re_sdk_types::blueprint::archetypes::{TextLogColumns, TextLogFormat, TextLogRows};
use re_sdk_types::blueprint::components::{Enabled, TextLogColumn, TimelineColumn};
use re_sdk_types::blueprint::encodings as bp_encodings;
use re_sdk_types::components::TextLogLevel;
use re_sdk_types::{View as _, ViewClassIdentifier, encodings};
use re_ui::list_item::LabelContent;
use re_ui::{Help, TableStyle, UiExt as _};
use re_viewer_context::{
    IdentifiedViewSystem as _, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewContext,
    ViewId, ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewerContext, level_to_rich_text,
};
use re_viewport_blueprint::ViewProperty;

use super::visualizer_system::{TextLogOutput, TextLogSystem};
use crate::row_layout::{RowLayout, VisibleRows};

// TODO(andreas): This should be a blueprint component.
#[derive(Clone, PartialEq, Eq, Default, re_byte_size::SizeBytes)]
pub struct TextViewState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// text entry window however they please when the time cursor isn't moving.
    latest_time: i64,

    /// Row just below the time cursor on the previous render.
    ///
    /// We auto-scroll whenever this changes so the view tracks the latest-at
    /// row as new (possibly out-of-order) data streams in. This handles both
    /// the initial catch-up to a programmatic `SetTime` (e.g. a `#when` URL
    /// anchor pointing past the data loaded so far) and any later arrival
    /// that lands at or before the cursor.
    last_anchor_row: Option<u64>,

    seen_levels: BTreeSet<String>,
}

impl ViewState for TextViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn heap_size_bytes(&self) -> u64 {
        re_byte_size::SizeBytes::heap_size_bytes(self)
    }
}

#[derive(Default)]
pub struct TextView;

type ViewType = re_sdk_types::blueprint::views::TextLogView;

impl ViewClass for TextView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Text log"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_LOG
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Text log view")
            .docs_link("https://rerun.io/docs/reference/types/views/text_log_view")
            .markdown(
                "TextLog entries over time.

Filter message types and toggle column visibility in a selection panel.",
            )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_array_fallback_provider(
            TextLogColumns::descriptor_timeline_columns().component,
            |ctx| {
                let active_timeline = ctx.viewer_ctx().time_ctrl.timeline_name();
                vec![TimelineColumn(bp_encodings::TimelineColumn {
                    visible: true.into(),
                    timeline: active_timeline.as_str().into(),
                })]
            },
        );

        system_registry.register_array_fallback_provider(
            TextLogColumns::descriptor_text_log_columns().component,
            |_ctx| {
                [
                    bp_encodings::TextLogColumnKind::EntityPath,
                    bp_encodings::TextLogColumnKind::LogLevel,
                    bp_encodings::TextLogColumnKind::Body,
                ]
                .map(|kind| {
                    TextLogColumn(bp_encodings::TextLogColumn {
                        kind,
                        visible: true.into(),
                    })
                })
            },
        );

        system_registry.register_array_fallback_provider(
            TextLogRows::descriptor_filter_by_log_level().component,
            |ctx| {
                let Ok(state) = ctx.view_state().downcast_ref::<TextViewState>() else {
                    re_log::error_once!(
                        "Failed to get `TextViewState` in text log view fallback, this is a bug."
                    );

                    return Vec::new();
                };
                state
                    .seen_levels
                    .iter()
                    .map(|lvl| TextLogLevel(encodings::Utf8::from(lvl.as_str())))
                    .collect::<Vec<_>>()
            },
        );
        system_registry.register_visualizer::<TextLogSystem>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<TextViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        Some(2.0) // Make text logs wide
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();

        // Spawn a single log view at the root if there's any text logs around anywhere.
        // Checking indicators is enough, since we know that this is enough to infer visualizability here.
        if ctx
            .indicated_entities_per_visualizer
            .get(&TextLogSystem::identifier())
            .is_some_and(|entities| entities.iter().any(include_entity))
        {
            ViewSpawnHeuristics::root()
        } else {
            ViewSpawnHeuristics::empty()
        }
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TextViewState>()?;

        ui.list_item_scope("text_log_selection_ui", |ui| {
            let ctx = self.view_context(ctx, view_id, state, space_origin);
            re_view::view_property_ui::<TextLogColumns>(&ctx, ui);
            view_property_ui_rows(&ctx, ui);
            re_view::view_property_ui::<TextLogFormat>(&ctx, ui);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<re_viewer_context::ViewClassUiOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let tokens = ui.tokens();
        let table_style = TableStyle::Dense;
        let state = state.downcast_mut::<TextViewState>()?;
        let output = system_output
            .visualizer_data_or_default::<TextLogOutput>(TextLogSystem::identifier())?;
        let output: &TextLogOutput = &output;

        let view_ctx = self.view_context(ctx, query.view_id, state, query.space_origin);
        let columns_property = ViewProperty::from_archetype::<TextLogColumns>(&view_ctx);
        let format_property = ViewProperty::from_archetype::<TextLogFormat>(&view_ctx);

        let monospace_body = format_property.component_or_fallback::<Enabled>(
            &view_ctx,
            TextLogFormat::descriptor_monospace_body().component,
        )?;
        let columns = columns_property.component_array_or_fallback::<TextLogColumn>(
            &view_ctx,
            TextLogColumns::descriptor_text_log_columns().component,
        )?;

        let timeline_columns = columns_property.component_array_or_fallback::<TimelineColumn>(
            &view_ctx,
            TextLogColumns::descriptor_timeline_columns().component,
        )?;

        let time = ctx.time_ctrl.time_i64().unwrap_or(state.latest_time);

        // Everything about *where* rows live comes from the visualizer's row layout, derived
        // from chunk-level metadata; only the rows that end up on screen are resolved from the
        // actual row data (in `prepare()`).
        // `None` means the visualizer didn't run, i.e. the view has no active instructions.
        let layout = output.layout.as_ref();

        if let Some(layout) = layout {
            state.seen_levels.extend(layout.levels().iter().cloned());
        }

        let num_rows = layout.map_or(0, RowLayout::num_rows);
        let rows_before = |t: TimeInt| layout.map_or(0, |layout| layout.rows_before(t));

        let scroll_row = rows_before(TimeInt::new_temporal(time));
        let anchor_row = rows_before(TimeInt::new_temporal(time).inc());

        // Auto-scroll when the time cursor moves, or whenever the row below the cursor shifts
        // because new (possibly out-of-order) data landed at or before the cursor.
        let time_cursor_moved = state.latest_time != time;
        let anchor_moved = state.last_anchor_row != Some(anchor_row);
        let scroll_to = (time_cursor_moved || anchor_moved).then_some(scroll_row);
        state.last_anchor_row = Some(anchor_row);
        state.latest_time = time;

        // Draw the current time indicator when the active timeline is shown as a column.
        let marker_row = (ctx.time_ctrl.time_int().is_some()
            && timeline_columns
                .iter()
                .any(|col| *col.visible && col.timeline.as_str() == query.timeline.as_str()))
        .then_some(anchor_row);

        let mut column_kinds = Vec::new();
        let mut table_columns = Vec::new();
        for col in &timeline_columns {
            if !*col.visible {
                continue;
            }
            if let Some(timeline) =
                TimelineName::try_new(col.timeline.as_str()).ok_or_log_error_once()
            {
                column_kinds.push(ColumnKind::Timeline(timeline));
                table_columns.push(text_log_column(110.0, 60.0));
            }
        }
        for col in &columns {
            if !*col.visible {
                continue;
            }
            let (width, min_width) = match col.kind {
                bp_encodings::TextLogColumnKind::EntityPath => (120.0, 60.0),
                bp_encodings::TextLogColumnKind::LogLevel => (50.0, 44.0),
                bp_encodings::TextLogColumnKind::Body => (400.0, 100.0),
            };
            column_kinds.push(ColumnKind::Kind(col.kind));
            table_columns.push(text_log_column(width, min_width));
        }

        let mut delegate = TextLogTableDelegate {
            ctx,
            table_style,
            row_height: tokens.table_row_height(table_style),
            column_kinds: &column_kinds,
            monospace_body: **monospace_body,
            layout,
            visible: VisibleRows::default(),
            num_rows,
            marker_row,
        };

        egui::Frame {
            inner_margin: tokens.view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            apply_table_style_fixes(ui.style_mut());

            let mut table = egui_table::Table::new()
                .id_salt(egui::Id::new("text_log").with(query.view_id))
                .columns(table_columns)
                .auto_size_mode(egui_table::AutoSizeMode::Always)
                .headers(vec![egui_table::HeaderRow::new(
                    tokens.table_header_height(),
                )])
                .num_rows(num_rows);

            if let Some(scroll_to) = scroll_to {
                table = table.scroll_to_row(scroll_to, Some(egui::Align::Center));
            }

            re_tracing::profile_scope!("render table");
            table.show(ui, &mut delegate);
        });

        Ok(Default::default())
    }
}

// ---

enum ColumnKind {
    Timeline(TimelineName),
    Kind(bp_encodings::TextLogColumnKind),
}

/// A resizable column that starts out `width` wide but may be squeezed down to `min_width`
/// when the view is too narrow to fit every column at its preferred width.
fn text_log_column(width: f32, min_width: f32) -> egui_table::Column {
    egui_table::Column::new(width)
        .range(egui::Rangef::new(min_width, f32::INFINITY))
        .resizable(true)
}

struct TextLogTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    table_style: TableStyle,
    row_height: f32,
    column_kinds: &'a [ColumnKind],
    monospace_body: bool,
    layout: Option<&'a RowLayout>,
    num_rows: u64,

    /// Paint the current time indicator at the top of this row
    /// (or at the bottom of the last row if this is one past the end).
    marker_row: Option<u64>,

    /// The rows on screen this frame, resolved from the layout in [`Self::prepare`].
    visible: VisibleRows,
}

impl egui_table::TableDelegate for TextLogTableDelegate<'_> {
    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        self.visible = self.layout.map_or_else(VisibleRows::default, |layout| {
            layout.visible_rows(info.visible_rows.clone())
        });
    }

    fn default_row_height(&self) -> f32 {
        self.row_height
    }

    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        let col_nr = cell.col_range.start;

        egui::Frame::new()
            .inner_margin(ui.tokens().header_cell_margin(self.table_style))
            .show(ui, |ui| match &self.column_kinds[col_nr] {
                ColumnKind::Timeline(timeline) => {
                    timeline_button(&self.ctx.app_ctx, ui, timeline);
                }
                ColumnKind::Kind(kind) => {
                    ui.strong(kind.name());
                }
            });

        let rect = ui.max_rect().round_to_pixels(ui.pixels_per_point());
        self.paint_column_separator(ui, col_nr, rect);

        // A single subtle line under the header, tiled across the columns.
        // Note: `apply_table_style_fixes` blanks `noninteractive.bg_stroke`, so use the token.
        ui.painter().hline(
            rect.x_range(),
            rect.max.y - re_dataframe_ui::CELL_SEPARATOR_STROKE_OFFSET,
            egui::Stroke::new(1.0, ui.tokens().table_interaction_noninteractive_bg_stroke),
        );
    }

    fn row_ui(&mut self, ui: &mut egui::Ui, row_nr: u64) {
        let paint_marker_at = if self.marker_row == Some(row_nr) {
            Some(ui.max_rect().top())
        } else if row_nr + 1 == self.num_rows && self.marker_row == Some(self.num_rows) {
            // The time cursor is past all rows: draw the marker below the last one.
            Some(ui.max_rect().bottom())
        } else {
            None
        };

        if let Some(y) = paint_marker_at {
            ui.painter().hline(
                ui.max_rect().x_range(),
                y,
                (1.0, ui.tokens().strong_fg_color),
            );
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let col_nr = cell.col_nr;

        egui::Frame::new()
            .inner_margin(ui.tokens().table_cell_margin(self.table_style))
            .show(ui, |ui| {
                let Some((layout, row)) = Option::zip(self.layout, self.visible.get(cell.row_nr))
                else {
                    // Can only happen if the table requests a row outside the prepared range.
                    ui.weak("…");
                    return;
                };

                match &self.column_kinds[col_nr] {
                    ColumnKind::Timeline(timeline) => {
                        let row_time = layout.row_time(row, timeline);
                        item_ui::time_button(self.ctx, ui, timeline, row_time);
                    }
                    ColumnKind::Kind(bp_encodings::TextLogColumnKind::EntityPath) => {
                        item_ui::entity_path_button(
                            &self.ctx.active_recording_store_view_context(),
                            ui,
                            None,
                            layout.row_data(row).entity_path,
                        );
                    }
                    ColumnKind::Kind(bp_encodings::TextLogColumnKind::LogLevel) => {
                        if let Some(lvl) = layout.row_data(row).level {
                            ui.label(level_to_rich_text(ui, lvl));
                        } else {
                            ui.label("-");
                        }
                    }
                    ColumnKind::Kind(bp_encodings::TextLogColumnKind::Body) => {
                        let data = layout.row_data(row);

                        // Rows have a fixed height; show only the first line of multi-line bodies.
                        let body = data.body.unwrap_or_default();
                        let (first_line, truncated) = match body.split_once('\n') {
                            Some((first_line, _)) => (first_line, true),
                            None => (body, false),
                        };

                        let mut text = if truncated {
                            egui::RichText::new(format!("{first_line} …"))
                        } else {
                            egui::RichText::new(first_line)
                        };

                        if self.monospace_body {
                            text = text.monospace();
                        }
                        if let Some(color) = data.color {
                            text = text.color(color);
                        }

                        ui.label(text).on_hover_text(body);
                    }
                }
            });

        let rect = ui.max_rect().round_to_pixels(ui.pixels_per_point());
        self.paint_column_separator(ui, col_nr, rect);
    }
}

impl TextLogTableDelegate<'_> {
    /// A vertical line between columns (but not after the last one), like the old
    /// `egui_extras`-based renderer had.
    fn paint_column_separator(&self, ui: &egui::Ui, col_nr: usize, rect: egui::Rect) {
        if col_nr + 1 < self.column_kinds.len() {
            ui.painter().vline(
                rect.max.x - re_dataframe_ui::CELL_SEPARATOR_STROKE_OFFSET,
                rect.y_range(),
                egui::Stroke::new(1.0, ui.tokens().table_interaction_noninteractive_bg_stroke),
            );
        }
    }
}

/// We need this to be a custom ui to be able to use the view state to get seen text log levels.
///
/// This could potentially be avoided if we could add component ui's from this crate.
fn view_property_ui_rows(ctx: &ViewContext<'_>, ui: &mut egui::Ui) {
    let property = ViewProperty::from_archetype::<TextLogRows>(ctx);

    let reflection = ctx.viewer_ctx.reflection();
    let Some(reflection) = reflection.archetypes.get(&property.archetype_name) else {
        ui.error_label(format!(
            "Missing reflection data for archetype {}.",
            property.archetype_name
        ));
        return;
    };

    let query_ctx = property.query_context(ctx);

    let sub_prop_ui = |ui: &mut egui::Ui| {
        for field in &reflection.fields {
            if field
                .component_descriptor(property.archetype_name)
                .component
                == TextLogRows::descriptor_filter_by_log_level().component
            {
                re_view::view_property_component_ui_custom(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                    &|_| {},
                    Some(&|ui| {
                        let Ok(state) = ctx.view_state.downcast_ref::<TextViewState>() else {
                            ui.error_label("Failed to get text log view state");
                            return;
                        };

                        let Ok(levels) = property.component_array_or_fallback::<TextLogLevel>(
                            ctx,
                            TextLogRows::descriptor_filter_by_log_level().component,
                        ) else {
                            ui.error_label("Failed to query text log levels component");
                            return;
                        };

                        let mut new_levels = std::iter::chain(
                            state.seen_levels.iter().map(|s| {
                                let level_active = levels.iter().any(|l| l.as_str() == s);
                                (s.clone(), level_active)
                            }),
                            levels
                                .iter()
                                .filter(|lvl| !state.seen_levels.contains(lvl.as_str()))
                                .map(|lvl| (lvl.as_str().to_owned(), true)),
                        )
                        .collect::<Vec<_>>();

                        let mut any_change = false;
                        for (lvl, active) in &mut new_levels {
                            any_change |= ui
                                .re_checkbox(active, level_to_rich_text(ui, lvl))
                                .changed();
                        }

                        if any_change {
                            if new_levels.iter().all(|(_, active)| *active) {
                                // Nothing is filtered out: remove the filter entirely, so that
                                // levels showing up later stay visible and the view stays on
                                // the cheaper unfiltered path.
                                property.reset_blueprint_component(
                                    ctx.viewer_ctx,
                                    TextLogRows::descriptor_filter_by_log_level(),
                                );
                            } else {
                                let log_levels: Vec<_> = new_levels
                                    .into_iter()
                                    .filter(|(_, active)| *active)
                                    .map(|(lvl, _)| TextLogLevel(lvl.into()))
                                    .collect();

                                property.save_blueprint_component(
                                    ctx.viewer_ctx,
                                    &TextLogRows::descriptor_filter_by_log_level(),
                                    &log_levels,
                                );
                            }
                        }
                    }),
                );
            } else {
                re_view::view_property_component_ui(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                );
            }
        }
    };

    if reflection.fields.len() == 1 {
        sub_prop_ui(ui);
    } else {
        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(property.archetype_name.full_name()),
                true,
                LabelContent::new(reflection.display_name),
                sub_prop_ui,
            );
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| TextView.help(ctx));
}
