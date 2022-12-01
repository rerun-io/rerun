use std::collections::BTreeMap;

use egui::{Color32, NumExt as _, RichText};
use re_data_store::{ObjPath, Timeline};
use re_log_types::{LogMsg, TimePoint};

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
        .time_query()
        .map_or(state.latest_time, |q| match q {
            re_data_store::TimeQuery::LatestAt(time) => time,
            re_data_store::TimeQuery::Range(range) => *range.start(),
        });

    // Did the time cursor move since last time?
    // - If it did, time to autoscroll appropriately.
    // - Otherwise, let the user scroll around freely!
    let time_cursor_moved = state.latest_time != time;
    let scroll_to_row = time_cursor_moved.then(|| {
        crate::profile_scope!("TextEntryState - search scroll time");
        scene
            .text_entries
            .partition_point(|entry| entry.time < time)
    });

    state.latest_time = time;

    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.label(format!("{} text entries", scene.text_entries.len()));
        ui.separator();

        egui::ScrollArea::horizontal().show(ui, |ui| {
            crate::profile_scope!("render table");
            show_table(ctx, ui, state, &scene.text_entries, scroll_to_row);
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

pub(crate) fn text_filters_ui(ui: &mut egui::Ui, state: &mut ViewTextState) -> egui::Response {
    ui.vertical(|ui| state.filters.ui(ui)).response
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
    let Some(msg) = ctx.log_db.get_log_msg(&entry.msg_id) else {
        re_log::warn_once!("Missing LogMsg for {:?}", entry.obj_path.obj_type_path());
        return None;
    };

    let LogMsg::DataMsg(data_msg) = msg else {
        re_log::warn_once!(
            "LogMsg must be a DataMsg ({:?})",
            entry.obj_path.obj_type_path()
        );
        return None;
    };

    Some(data_msg.time_point.clone())
}

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
fn show_table(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTextState,
    text_entries: &[TextEntry],
    scroll_to_row: Option<usize>,
) {
    let current_timeline = *ctx.rec_cfg.time_ctrl.timeline();
    let current_time = ctx.rec_cfg.time_ctrl.time().map(|tr| tr.floor());

    let timelines = state
        .filters
        .col_timelines
        .iter()
        .filter_map(|(timeline, visible)| visible.then_some(timeline))
        .collect::<Vec<_>>();

    use egui_extras::Size;
    const ROW_HEIGHT: f32 = 18.0;
    const HEADER_HEIGHT: f32 = 20.0;

    let max_content_height = ui.available_height() - HEADER_HEIGHT;
    let item_spacing = ui.spacing().item_spacing;

    let mut table_builder = egui_extras::TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .scroll(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));

    if let Some(index) = scroll_to_row {
        let row_height_full = ROW_HEIGHT + item_spacing.y;
        let scroll_to_offset = index as f32 * row_height_full;

        // Scroll to center:
        let scroll_to_offset = scroll_to_offset - max_content_height / 2.0;

        // Don't over-scroll:
        let scroll_to_offset = scroll_to_offset.clamp(
            0.0,
            (text_entries.len() as f32 * row_height_full - max_content_height).at_least(0.0),
        );

        table_builder = table_builder.vertical_scroll_offset(scroll_to_offset);
    }

    let mut current_time_y = None;

    {
        // timeline(s)
        table_builder = table_builder.columns(Size::initial(100.0), timelines.len());

        // object path
        if state.filters.col_obj_path {
            table_builder = table_builder.column(Size::initial(100.0));
        }
        // log level
        if state.filters.col_log_level {
            table_builder = table_builder.column(Size::initial(100.0));
        }
        // body
        table_builder = table_builder.column(Size::remainder().at_least(100.0));
    }
    table_builder
        .header(HEADER_HEIGHT, |mut header| {
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
        .body(|body| {
            body.rows(ROW_HEIGHT, text_entries.len(), |index, mut row| {
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
                        if let Some(value) = time_point.0.get(timeline).copied() {
                            if let Some(current_time) = current_time {
                                if current_time_y.is_none()
                                    && *timeline == &current_timeline
                                    && value >= current_time
                                {
                                    current_time_y = Some(ui.max_rect().top());
                                }
                            }

                            ctx.time_button(ui, timeline, value);
                        }
                    });
                }

                // path
                if state.filters.col_obj_path {
                    row.col(|ui| {
                        ctx.obj_path_button(ui, &text_entry.obj_path);
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
                    if let Some(c) = text_entry.color {
                        let color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        ui.colored_label(color, &text_entry.body);
                    } else {
                        ui.label(&text_entry.body);
                    }
                });
            });
        });

    // TODO(cmc): this draws on top of the headers :(
    if let Some(current_time_y) = current_time_y {
        // Show that the current time is here:
        ui.painter().hline(
            ui.max_rect().x_range(),
            current_time_y,
            (1.0, Color32::WHITE),
        );
    }
}

fn level_to_rich_text(ui: &egui::Ui, lvl: &str) -> RichText {
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
