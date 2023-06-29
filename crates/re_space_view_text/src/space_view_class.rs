use std::collections::BTreeMap;

use re_data_ui::item_ui;
use re_log_types::{EntityPath, TimePoint, Timeline};
use re_viewer_context::{
    level_to_rich_text, SpaceViewClass, SpaceViewClassName, SpaceViewId, SpaceViewState,
    TypedScene, ViewerContext,
};

use super::scene_part::{SceneText, TextEntry};

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

impl SpaceViewClass for TextSpaceView {
    type State = TextSpaceViewState;
    type Context = ();
    type SceneParts = SceneText;
    type ScenePartData = ();

    fn name(&self) -> SpaceViewClassName {
        "Text".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TEXTBOX
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi, _state: &Self::State) -> egui::WidgetText {
        "Shows text entries over time.\nSelect the Space View for filtering options.".into()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        Some(2.0) // Make text logs wide
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) {
        let ViewTextFilters {
            col_timelines,
            col_entity_path,
            col_log_level,
            row_log_levels,
        } = &mut state.filters;

        ctx.re_ui.selection_grid(ui, "log_config").show(ui, |ui| {
            ctx.re_ui.grid_left_hand_label(ui, "Columns");
            ui.vertical(|ui| {
                for (timeline, visible) in col_timelines {
                    ui.checkbox(visible, timeline.name().to_string());
                }
                ui.checkbox(col_entity_path, "Entity path");
                ui.checkbox(col_log_level, "Log level");
            });
            ui.end_row();

            ctx.re_ui.grid_left_hand_label(ui, "Level Filter");
            ui.vertical(|ui| {
                for (log_level, visible) in row_log_levels {
                    ui.checkbox(visible, level_to_rich_text(ui, log_level));
                }
            });
            ui.end_row();

            ctx.re_ui.grid_left_hand_label(ui, "Text style");
            ui.vertical(|ui| {
                ui.radio_value(&mut state.monospace, false, "Proportional");
                ui.radio_value(&mut state.monospace, true, "Monospace");
            });
            ui.end_row();
        });
    }

    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        scene: &mut TypedScene<Self>,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) {
        let scene = &scene.parts;

        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            // Update filters if necessary.
            state.filters.update(ctx, &scene.text_entries);

            let time = ctx
                .rec_cfg
                .time_ctrl
                .time_i64()
                .unwrap_or(state.latest_time);

            // Did the time cursor move since last time?
            // - If it did, autoscroll to the text log to reveal the current time.
            // - Otherwise, let the user scroll around freely!
            let time_cursor_moved = state.latest_time != time;
            let scroll_to_row = time_cursor_moved.then(|| {
                re_tracing::profile_scope!("TextEntryState - search scroll time");
                scene
                    .text_entries
                    .partition_point(|te| te.time.unwrap_or(i64::MIN) < time)
            });

            state.latest_time = time;

            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    re_tracing::profile_scope!("render table");
                    table_ui(ctx, ui, state, &scene.text_entries, scroll_to_row);
                })
            });
        });
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
    pub row_log_levels: BTreeMap<String, bool>,
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
    fn update(&mut self, ctx: &mut ViewerContext<'_>, text_entries: &[TextEntry]) {
        re_tracing::profile_function!();

        let Self {
            col_timelines,
            col_entity_path: _,
            col_log_level: _,
            row_log_levels,
        } = self;

        for timeline in ctx.store_db.timelines() {
            col_timelines.entry(*timeline).or_insert(true);
        }

        for level in text_entries.iter().filter_map(|te| te.level.as_ref()) {
            row_log_levels.entry(level.clone()).or_insert(true);
        }
    }
}

// ---

fn get_time_point(ctx: &ViewerContext<'_>, entry: &TextEntry) -> Option<TimePoint> {
    if let Some(time_point) = ctx
        .store_db
        .entity_db
        .data_store
        .get_msg_metadata(&entry.row_id)
    {
        Some(time_point.clone())
    } else {
        re_log::warn_once!("Missing LogMsg for {:?}", entry.entity_path);
        None
    }
}

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
fn table_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut TextSpaceViewState,
    text_entries: &[TextEntry],
    scroll_to_row: Option<usize>,
) {
    let timelines = state
        .filters
        .col_timelines
        .iter()
        .filter_map(|(timeline, visible)| visible.then_some(timeline))
        .collect::<Vec<_>>();

    use egui_extras::Column;

    let global_timeline = *ctx.rec_cfg.time_ctrl.timeline();
    let global_time = ctx.rec_cfg.time_ctrl.time_int();

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
        .header(re_ui::ReUi::table_header_height(), |mut header| {
            re_ui::ReUi::setup_table_header(&mut header);
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
            re_ui::ReUi::setup_table_body(&mut body);

            body_clip_rect = Some(body.max_rect());

            let row_heights = text_entries.iter().map(calc_row_height);
            body.heterogeneous_rows(row_heights, |index, mut row| {
                let text_entry = &text_entries[index];

                // NOTE: `try_from_props` is where we actually fetch data from the underlying
                // store, which is a costly operation.
                // Doing this here guarantees that it only happens for visible rows.
                let Some(time_point) = get_time_point(ctx, text_entry) else {
                    row.col(|ui| {
                        ui.colored_label(
                            egui::Color32::RED,
                            "<failed to load TextEntry from data store>",
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
                        item_ui::entity_path_button(ctx, ui, None, &text_entry.entity_path);
                    });
                }

                // level
                if state.filters.col_log_level {
                    row.col(|ui| {
                        if let Some(lvl) = &text_entry.level {
                            ui.label(level_to_rich_text(ui, lvl));
                        } else {
                            ui.label("-");
                        }
                    });
                }

                // body
                row.col(|ui| {
                    let mut text = egui::RichText::new(&text_entry.body);

                    if state.monospace {
                        text = text.monospace();
                    }
                    if let Some([r, g, b, a]) = text_entry.color {
                        text = text.color(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
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

fn calc_row_height(entry: &TextEntry) -> f32 {
    // Simple, fast, ugly, and functional
    let num_newlines = entry.body.bytes().filter(|&c| c == b'\n').count();
    let num_rows = 1 + num_newlines;
    num_rows as f32 * re_ui::ReUi::table_line_height()
}
