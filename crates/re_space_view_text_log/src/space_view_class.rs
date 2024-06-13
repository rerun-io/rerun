use std::collections::BTreeMap;

use re_data_ui::item_ui;
use re_entity_db::EntityProperties;
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_types::View;
use re_types::{components::TextLogLevel, SpaceViewClassIdentifier};
use re_ui::UiExt as _;
use re_viewer_context::{
    level_to_rich_text, IdentifiedViewSystem as _, SpaceViewClass, SpaceViewClassRegistryError,
    SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewState, SpaceViewStateExt,
    SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
};

use super::visualizer_system::{Entry, TextLogSystem};

// TODO(andreas): This should be a blueprint component.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct TextSpaceViewState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// text entry window however they please when the time cursor isn't moving.
    latest_time: i64,

    pub filters: ViewTextFilters,

    monospace: bool,
}

impl SpaceViewState for TextSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct TextSpaceView;

type ViewType = re_types::blueprint::views::TextLogView;

impl SpaceViewClass for TextSpaceView {
    fn identifier() -> SpaceViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Text log"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_LOG
    }

    fn help_text(&self, _egui_ctx: &egui::Context) -> egui::WidgetText {
        "Shows TextLog entries over time.\nSelect the Space View for filtering options.".into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_visualizer::<TextLogSystem>()
    }

    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<TextSpaceViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        Some(2.0) // Make text logs wide
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();

        // Spawn a single log view at the root if there's any text logs around anywhere.
        // Checking indicators is enough, since we know that this is enough to infer visualizability here.
        if ctx
            .indicated_entities_per_visualizer
            .get(&TextLogSystem::identifier())
            .map_or(true, |entities| entities.is_empty())
        {
            SpaceViewSpawnHeuristics::default()
        } else {
            SpaceViewSpawnHeuristics::root()
        }
    }

    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        let state = state.downcast_mut::<TextSpaceViewState>()?;

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
                    ui.re_checkbox(visible, timeline.name().to_string());
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
        state: &mut dyn SpaceViewState,
        _root_entity_properties: &EntityProperties,
        _query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<TextSpaceViewState>()?;
        let text = system_output.view_systems.get::<TextLogSystem>()?;

        // TODO(andreas): Should filter text entries in the part-system instead.
        // this likely requires a way to pass state into a context.
        let entries = text
            .entries
            .iter()
            .filter(|te| {
                te.level
                    .as_ref()
                    .map_or(true, |lvl| state.filters.is_log_level_visible(lvl))
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
    pub col_timelines: BTreeMap<Timeline, bool>,
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

        for timeline in ctx.recording().timelines() {
            col_timelines.entry(*timeline).or_insert(true);
        }

        for level in entries.iter().filter_map(|te| te.level.as_ref()) {
            row_log_levels.entry(level.clone()).or_insert(true);
        }
    }
}

// ---

fn get_time_point(ctx: &ViewerContext<'_>, entry: &Entry) -> Option<TimePoint> {
    if let Some((time_point, _)) = ctx.recording_store().row_metadata(&entry.row_id) {
        Some(time_point.clone())
    } else {
        re_log::warn_once!("Missing metadata for {:?}", entry.entity_path);
        None
    }
}

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
fn table_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &TextSpaceViewState,
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
        .auto_shrink([false; 2]) // expand to take up the whole Space View
        .min_scrolled_height(0.0) // we can go as small as we need to be in order to fit within the space view!
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

                // NOTE: `try_from_props` is where we actually fetch data from the underlying
                // store, which is a costly operation.
                // Doing this here guarantees that it only happens for visible rows.
                let Some(time_point) = get_time_point(ctx, entry) else {
                    row.col(|ui| {
                        ui.colored_label(
                            egui::Color32::RED,
                            "<failed to load TextLog from data store>",
                        );
                    });
                    return;
                };

                // timeline(s)
                for timeline in &timelines {
                    row.col(|ui| {
                        if let Some(row_time) = time_point.get(timeline).copied() {
                            item_ui::time_button(ctx, ui, timeline, row_time);

                            if let Some(global_time) = global_time {
                                if *timeline == &global_timeline {
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
            (1.0, egui::Color32::WHITE),
        );
    }
}

fn calc_row_height(entry: &Entry) -> f32 {
    // Simple, fast, ugly, and functional
    let num_newlines = entry.body.bytes().filter(|&c| c == b'\n').count();
    let num_rows = 1 + num_newlines;
    num_rows as f32 * re_ui::DesignTokens::table_line_height()
}
