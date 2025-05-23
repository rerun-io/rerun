use std::collections::BTreeMap;

use re_data_ui::item_ui;
use re_log_types::{EntityPath, TimelineName};
use re_types::View as _;
use re_types::{ViewClassIdentifier, components::TextLogLevel};
use re_ui::{Help, UiExt as _};
use re_viewer_context::{
    IdentifiedViewSystem as _, ViewClass, ViewClassRegistryError, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext,
    level_to_rich_text,
};

use super::visualizer_system::{Entry, TextLogSystem};

// TODO(andreas): This should be a blueprint component.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct TextViewState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// text entry window however they please when the time cursor isn't moving.
    latest_time: i64,

    pub filters: ViewTextFilters,

    monospace: bool,
}

impl ViewState for TextViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TextView;

type ViewType = re_types::blueprint::views::TextLogView;

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

    fn help(&self, _egui_ctx: &egui::Context) -> Help {
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
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        _view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TextViewState>()?;

        let ViewTextFilters {
            col_timelines,
            col_entity_path,
            col_log_level,
            row_log_levels,
        } = &mut state.filters;

        ui.selection_grid("log_config").show(ui, |ui| {
            ui.grid_left_hand_label("Columns");
            ui.vertical(|ui| {
                for (timeline, visible) in col_timelines {
                    ui.re_checkbox(visible, timeline.to_string());
                }
                ui.re_checkbox(col_entity_path, "Entity path");
                ui.re_checkbox(col_log_level, "Log level");
            });
            ui.end_row();

            ui.grid_left_hand_label("Level Filter");
            ui.vertical(|ui| {
                for (log_level, visible) in row_log_levels {
                    ui.re_checkbox(visible, level_to_rich_text(ui, log_level));
                }
            });
            ui.end_row();

            ui.grid_left_hand_label("Text style");
            ui.vertical(|ui| {
                ui.re_radio_value(&mut state.monospace, false, "Proportional");
                ui.re_radio_value(&mut state.monospace, true, "Monospace");
            });
            ui.end_row();
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,

        _query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<TextViewState>()?;
        let text = system_output.view_systems.get::<TextLogSystem>()?;

        // TODO(andreas): Should filter text entries in the part-system instead.
        // this likely requires a way to pass state into a context.
        let entries = text
            .entries
            .iter()
            .filter(|te| {
                te.level
                    .as_ref()
                    .is_none_or(|lvl| state.filters.is_log_level_visible(lvl))
            })
            .collect::<Vec<_>>();

        egui::Frame {
            inner_margin: re_ui::DesignTokens::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            // Update filters if necessary.
            state.filters.update(ctx, &entries);

            let time = ctx
                .rec_cfg
                .time_ctrl
                .read()
                .time_i64()
                .unwrap_or(state.latest_time);

            // Did the time cursor move since last time?
            // - If it did, autoscroll to the text log to reveal the current time.
            // - Otherwise, let the user scroll around freely!
            let time_cursor_moved = state.latest_time != time;
            let scroll_to_row = time_cursor_moved.then(|| {
                re_tracing::profile_scope!("search scroll time");
                entries.partition_point(|te| te.time.as_i64() < time)
            });

            state.latest_time = time;

            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    re_tracing::profile_scope!("render table");
                    table_ui(ctx, ui, state, &entries, scroll_to_row);
                })
            });
        });

        Ok(())
    }
}

// --- Filters ---

// TODO(cmc): implement "body contains <value>" filter.
// TODO(cmc): beyond filters, it'd be nice to be able to swap columns at some point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewTextFilters {
    // Column filters: which columns should be visible?
    // Timelines are special: each one has a dedicated column.
    pub col_timelines: BTreeMap<TimelineName, bool>,
    pub col_entity_path: bool,
    pub col_log_level: bool,

    // Row filters: which rows should be visible?
    pub row_log_levels: BTreeMap<TextLogLevel, bool>,
}

impl Default for ViewTextFilters {
    fn default() -> Self {
        Self {
            col_entity_path: true,
            col_log_level: true,
            col_timelines: Default::default(),
            row_log_levels: Default::default(),
        }
    }
}

impl ViewTextFilters {
    pub fn is_log_level_visible(&self, level: &str) -> bool {
        self.row_log_levels.get(level).copied().unwrap_or(true)
    }

    // Checks whether new values are available for any of the filters, and updates everything
    // accordingly.
    fn update(&mut self, ctx: &ViewerContext<'_>, entries: &[&Entry]) {
        re_tracing::profile_function!();

        let Self {
            col_timelines,
            col_entity_path: _,
            col_log_level: _,
            row_log_levels,
        } = self;

        for &timeline in ctx.recording().timelines().keys() {
            col_timelines.entry(timeline).or_insert(true);
        }

        for level in entries.iter().filter_map(|te| te.level.as_ref()) {
            row_log_levels.entry(level.clone()).or_insert(true);
        }
    }
}

// ---

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
fn table_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &TextViewState,
    entries: &[&Entry],
    scroll_to_row: Option<usize>,
) {
    let timelines = state
        .filters
        .col_timelines
        .iter()
        .filter_map(|(timeline, visible)| visible.then_some(timeline))
        .collect::<Vec<_>>();

    use egui_extras::Column;

    let (global_timeline, global_time) = {
        let time_ctrl = ctx.rec_cfg.time_ctrl.read();
        (*time_ctrl.timeline(), time_ctrl.time_int())
    };

    let mut table_builder = egui_extras::TableBuilder::new(ui)
        .resizable(true)
        .vscroll(true)
        .auto_shrink([false; 2]) // expand to take up the whole View
        .min_scrolled_height(0.0) // we can go as small as we need to be in order to fit within the view!
        .max_scroll_height(f32::INFINITY) // Fill up whole height
        .cell_layout(egui::Layout::left_to_right(egui::Align::TOP));

    if let Some(scroll_to_row) = scroll_to_row {
        table_builder = table_builder.scroll_to_row(scroll_to_row, Some(egui::Align::Center));
    }

    let mut body_clip_rect = None;
    let mut current_time_y = None; // where to draw the current time indicator cursor

    {
        // timeline(s)
        table_builder =
            table_builder.columns(Column::auto().clip(true).at_least(32.0), timelines.len());

        // entity path
        if state.filters.col_entity_path {
            table_builder = table_builder.column(Column::auto().clip(true).at_least(32.0));
        }
        // log level
        if state.filters.col_log_level {
            table_builder = table_builder.column(Column::auto().at_least(30.0));
        }
        // body
        table_builder = table_builder.column(Column::remainder().at_least(100.0));
    }
    table_builder
        .header(re_ui::DesignTokens::table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
            for timeline in &timelines {
                header.col(|ui| {
                    item_ui::timeline_button(ctx, ui, timeline);
                });
            }
            if state.filters.col_entity_path {
                header.col(|ui| {
                    ui.strong("Entity path");
                });
            }
            if state.filters.col_log_level {
                header.col(|ui| {
                    ui.strong("Level");
                });
            }
            header.col(|ui| {
                ui.strong("Body");
            });
        })
        .body(|mut body| {
            re_ui::DesignTokens::setup_table_body(&mut body);

            body_clip_rect = Some(body.max_rect());

            let query = ctx.current_query();

            let row_heights = entries.iter().map(|te| calc_row_height(te));
            body.heterogeneous_rows(row_heights, |mut row| {
                let entry = &entries[row.index()];

                // timeline(s)
                for &timeline in &timelines {
                    row.col(|ui| {
                        let row_time = entry
                            .timepoint
                            .get(timeline)
                            .map(re_log_types::TimeInt::from)
                            .unwrap_or(re_log_types::TimeInt::STATIC);
                        item_ui::time_button(ctx, ui, timeline, row_time);

                        if let Some(global_time) = global_time {
                            if timeline == global_timeline.name() {
                                #[allow(clippy::comparison_chain)]
                                if global_time < row_time {
                                    // We've past the global time - it is thus above this row.
                                    if current_time_y.is_none() {
                                        current_time_y = Some(ui.max_rect().top());
                                    }
                                } else if global_time == row_time {
                                    // This row is exactly at the current time.
                                    // We could draw the current time exactly onto this row, but that would look bad,
                                    // so let's draw it under instead. It looks better in the "following" mode.
                                    current_time_y = Some(ui.max_rect().bottom());
                                }
                            }
                        }
                    });
                }

                // path
                if state.filters.col_entity_path {
                    row.col(|ui| {
                        item_ui::entity_path_button(
                            ctx,
                            &query,
                            ctx.recording(),
                            ui,
                            None,
                            &entry.entity_path,
                        );
                    });
                }

                // level
                if state.filters.col_log_level {
                    row.col(|ui| {
                        if let Some(lvl) = &entry.level {
                            ui.label(level_to_rich_text(ui, lvl));
                        } else {
                            ui.label("-");
                        }
                    });
                }

                // body
                row.col(|ui| {
                    let mut text = egui::RichText::new(entry.body.as_str());

                    if state.monospace {
                        text = text.monospace();
                    }
                    if let Some(color) = entry.color {
                        text = text.color(color);
                    }

                    ui.label(text);
                });
            });
        });

    // TODO(cmc): this draws on top of the headers :(
    if let (Some(body_clip_rect), Some(current_time_y)) = (body_clip_rect, current_time_y) {
        // Show that the current time is here:
        ui.painter().with_clip_rect(body_clip_rect).hline(
            ui.max_rect().x_range(),
            current_time_y,
            (1.0, ui.design_tokens().strong_fg_color),
        );
    }
}

fn calc_row_height(entry: &Entry) -> f32 {
    // Simple, fast, ugly, and functional
    let num_newlines = entry.body.bytes().filter(|&c| c == b'\n').count();
    let num_rows = 1 + num_newlines;
    num_rows as f32 * re_ui::DesignTokens::table_line_height()
}

#[test]
fn test_help_view() {
    re_viewer_context::test_context::TestContext::test_help_view(|ctx| TextView.help(ctx));
}
