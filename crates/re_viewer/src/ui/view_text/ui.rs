use std::collections::BTreeMap;

use egui::{Color32, RichText};

use re_data_store::{ObjPath, Timeline};
use re_log_types::TimePoint;

use crate::ViewerContext;

use super::{SceneText, TextEntry};

// --- Main view ---

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ViewTextState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// text entry window however they please when the time cursor isn't moving.
    latest_time: i64,

    pub filters: ViewTextFilters,

    monospace: bool,
}

impl ViewTextState {
    pub fn selection_ui(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label("Text style");
            ui.radio_value(&mut self.monospace, false, "Proportional");
            ui.radio_value(&mut self.monospace, true, "Monospace");
        });
        self.filters.ui(ui);
    }
}

pub(crate) fn view_text(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTextState,
    scene: &SceneText,
) -> egui::Response {
    crate::profile_function!();

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
        crate::profile_scope!("TextEntryState - search scroll time");
        scene
            .text_entries
            .partition_point(|te| te.time.unwrap_or(i64::MIN) < time)
    });

    state.latest_time = time;

    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.label(format!("{} text entries", scene.text_entries.len()));
        ui.separator();

        egui::ScrollArea::horizontal().show(ui, |ui| {
            crate::profile_scope!("render table");
            table_ui(ctx, ui, state, &scene.text_entries, scroll_to_row);
        })
    })
    .response
}

// --- Filters ---

// TODO(cmc): implement "body contains <value>" filter.
// TODO(cmc): beyond filters, it'd be nice to be able to swap columns at some point.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ViewTextFilters {
    // Column filters: which columns should be visible?
    // Timelines are special: each one has a dedicated column.
    pub col_timelines: BTreeMap<Timeline, bool>,
    pub col_obj_path: bool,
    pub col_log_level: bool,

    // Row filters: which rows should be visible?
    pub row_obj_paths: BTreeMap<ObjPath, bool>,
    pub row_log_levels: BTreeMap<String, bool>,
}

impl Default for ViewTextFilters {
    fn default() -> Self {
        Self {
            col_obj_path: true,
            col_log_level: true,
            col_timelines: Default::default(),
            row_obj_paths: Default::default(),
            row_log_levels: Default::default(),
        }
    }
}

impl ViewTextFilters {
    pub fn is_obj_path_visible(&self, obj_path: &ObjPath) -> bool {
        self.row_obj_paths.get(obj_path).copied().unwrap_or(true)
    }

    pub fn is_log_level_visible(&self, level: &str) -> bool {
        self.row_log_levels.get(level).copied().unwrap_or(true)
    }

    // Checks whether new values are available for any of the filters, and updates everything
    // accordingly.
    fn update(&mut self, ctx: &mut ViewerContext<'_>, text_entries: &[TextEntry]) {
        crate::profile_function!();

        let Self {
            col_timelines,
            col_obj_path: _,
            col_log_level: _,
            row_obj_paths,
            row_log_levels,
        } = self;

        for timeline in ctx.log_db.timelines() {
            col_timelines.entry(*timeline).or_insert(true);
        }

        for obj_path in text_entries.iter().map(|te| &te.obj_path) {
            row_obj_paths.entry(obj_path.clone()).or_insert(true);
        }

        for level in text_entries.iter().filter_map(|te| te.level.as_ref()) {
            row_log_levels.entry(level.clone()).or_insert(true);
        }
    }

    // Display the filter configuration UI (lotta checkboxes!).
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        crate::profile_function!();

        let Self {
            col_timelines,
            col_obj_path,
            col_log_level,
            row_obj_paths,
            row_log_levels,
        } = self;

        let has_obj_path_row_filters = row_obj_paths.values().filter(|v| **v).count() > 0;
        let has_log_lvl_row_filters = row_log_levels.values().filter(|v| **v).count() > 0;
        let has_any_row_filters = has_obj_path_row_filters || has_log_lvl_row_filters;

        let has_timeline_col_filters = col_timelines.values().filter(|v| **v).count() > 0;
        let has_any_col_filters = has_timeline_col_filters || *col_obj_path || *col_log_level;

        let clear_or_select = ["Select all", "Clear all"];

        // ---

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong("Visible columns");
                if ui
                    .button(clear_or_select[has_any_col_filters as usize])
                    .clicked()
                {
                    for v in col_timelines.values_mut() {
                        *v = !has_any_col_filters;
                    }
                    *col_obj_path = !has_any_col_filters;
                    *col_log_level = !has_any_col_filters;
                }
            });

            ui.add_space(2.0);

            for (timeline, visible) in col_timelines {
                ui.checkbox(visible, format!("Timeline: {}", timeline.name()));
            }
            ui.checkbox(col_obj_path, "Object path");
            ui.checkbox(col_log_level, "Log level");
        });

        ui.add_space(4.0);

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong("Row filters");
                if ui
                    .button(clear_or_select[has_any_row_filters as usize])
                    .clicked()
                {
                    for v in row_obj_paths.values_mut() {
                        *v = !has_any_row_filters;
                    }
                    for v in row_log_levels.values_mut() {
                        *v = !has_any_row_filters;
                    }
                }
            });

            ui.add_space(4.0);

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Object paths");
                    if ui
                        .button(clear_or_select[has_obj_path_row_filters as usize])
                        .clicked()
                    {
                        for v in row_obj_paths.values_mut() {
                            *v = !has_obj_path_row_filters;
                        }
                    }
                });
                for (obj_path, visible) in row_obj_paths {
                    ui.horizontal(|ui| {
                        ui.checkbox(visible, &obj_path.to_string());
                    });
                }
            });

            ui.add_space(4.0);

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Log levels");
                    if ui
                        .button(clear_or_select[has_log_lvl_row_filters as usize])
                        .clicked()
                    {
                        for v in row_log_levels.values_mut() {
                            *v = !has_log_lvl_row_filters;
                        }
                    }
                });
                for (log_level, visible) in row_log_levels {
                    ui.checkbox(visible, level_to_rich_text(ui, log_level));
                }
            });
        });
    }
}

// ---

fn get_time_point(ctx: &ViewerContext<'_>, entry: &TextEntry) -> Option<TimePoint> {
    if let Some(time_point) = ctx
        .log_db
        .obj_db
        .arrow_store
        .get_msg_metadata(&entry.msg_id)
    {
        Some(time_point.clone())
    } else {
        re_log::warn_once!("Missing LogMsg for {:?}", entry.obj_path.obj_type_path());
        None
    }
}

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
fn table_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTextState,
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

        // object path
        if state.filters.col_obj_path {
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
                    ctx.timeline_button(ui, timeline);
                });
            }
            if state.filters.col_obj_path {
                header.col(|ui| {
                    ui.strong("Object Path");
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
                let time_point = if let Some(time_point) = get_time_point(ctx, text_entry) {
                    time_point
                } else {
                    row.col(|ui| {
                        ui.colored_label(
                            Color32::RED,
                            "<failed to load TextEntry from data store>",
                        );
                    });
                    return;
                };

                // timeline(s)
                for timeline in &timelines {
                    row.col(|ui| {
                        if let Some(row_time) = time_point.get(timeline).copied() {
                            ctx.time_button(ui, timeline, row_time);

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
                if state.filters.col_obj_path {
                    row.col(|ui| {
                        ctx.obj_path_button(ui, None, &text_entry.obj_path);
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
                        text = text.color(Color32::from_rgba_unmultiplied(r, g, b, a));
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
            (1.0, Color32::WHITE),
        );
    }
}

fn calc_row_height(entry: &TextEntry) -> f32 {
    // Simple, fast, ugly, and functional
    let num_newlines = entry.body.bytes().filter(|&c| c == b'\n').count();
    let num_rows = 1 + num_newlines;
    num_rows as f32 * re_ui::ReUi::table_line_height()
}

pub fn level_to_rich_text(ui: &egui::Ui, lvl: &str) -> RichText {
    match lvl {
        "CRITICAL" => RichText::new(lvl)
            .color(Color32::WHITE)
            .background_color(ui.visuals().error_fg_color),
        "ERROR" => RichText::new(lvl).color(ui.visuals().error_fg_color),
        "WARN" => RichText::new(lvl).color(ui.visuals().warn_fg_color),
        "INFO" => RichText::new(lvl).color(Color32::LIGHT_GREEN),
        "DEBUG" => RichText::new(lvl).color(Color32::LIGHT_BLUE),
        "TRACE" => RichText::new(lvl).color(Color32::LIGHT_GRAY),
        _ => RichText::new(lvl).color(ui.visuals().text_color()),
    }
}
