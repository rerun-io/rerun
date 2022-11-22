use std::collections::BTreeMap;

use ahash::HashMap;
use egui::{Checkbox, Color32, Label, NumExt as _, Rect, RichText, TextStyle};

use re_data_store::{ObjPath, Timeline};
use re_log_types::{LogMsg, TimePoint};

use crate::ViewerContext;

use super::{SceneText, TextEntry};

// ---

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

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ViewTextFilters {
    // TODO:
    // - heuristics
    //
    // TODO:
    //
    // column pickers
    // --------------
    //
    // - timelines (log_time, frame_nr...)
    // - path
    // - level
    // - body
    //
    // custom pickers
    // --------------
    //
    // - timepoints => range
    // - path => all / one in particular / contains?
    // - level => lesser than / greater than / equals (multi choice)
    // - body => contains
    //
    // other
    // -----
    //
    // - reset button
    //
    // NOTE:
    // - Filters vs Selectors are in fact very different thing!

    // Column selectors
    pub show_timelines: BTreeMap<Timeline, bool>,
    pub show_obj_path: bool,
    pub show_log_level: bool,

    // Column filters
    pub filter_obj_paths: BTreeMap<ObjPath, bool>,
    pub filter_log_levels: BTreeMap<String, bool>,
}

impl ViewTextFilters {
    fn update(&mut self, ctx: &mut ViewerContext<'_>, text_entries: &[TextEntry]) {
        crate::profile_function!();

        for timeline in ctx.log_db.time_points.0.keys() {
            self.show_timelines.entry(timeline.clone()).or_insert(true);
        }

        for obj_path in text_entries.iter().map(|te| &te.obj_path) {
            self.filter_obj_paths
                .entry(obj_path.clone())
                .or_insert(true);
        }

        for level in text_entries.iter().filter_map(|te| te.level.as_ref()) {
            self.filter_log_levels.entry(level.clone()).or_insert(true);
        }
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        crate::profile_function!();

        ui.horizontal(|ui| {
            ui.strong("Column selections");
            if ui.button("Reset").clicked() {
                let Self {
                    show_timelines,
                    show_obj_path,
                    show_log_level,
                    filter_obj_paths: _,
                    filter_log_levels: _,
                } = self;

                for v in show_timelines.values_mut() {
                    *v = false;
                }
                *show_obj_path = false;
                *show_log_level = false;
            }
        });

        ui.add_space(2.0);

        for (timeline, visible) in &mut self.show_timelines {
            ui.checkbox(visible, format!("Timeline: {}", timeline.name()));
        }
        ui.checkbox(&mut self.show_obj_path, "Object path");
        ui.checkbox(&mut self.show_log_level, "Log level");

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.strong("Column filters");
            if ui.button("Clear all").clicked() {
                let Self {
                    show_timelines: _,
                    show_obj_path: _,
                    show_log_level: _,
                    filter_obj_paths,
                    filter_log_levels,
                } = self;

                for v in filter_obj_paths.values_mut() {
                    *v = false;
                }
                for v in filter_log_levels.values_mut() {
                    *v = false;
                }
            }
        });

        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.label("Object paths");
            if ui.button("Clear all").clicked() {
                for v in self.filter_obj_paths.values_mut() {
                    *v = false;
                }
            }
        });
        for (obj_path, visible) in &mut self.filter_obj_paths {
            ui.horizontal(|ui| {
                ui.checkbox(visible, "");
                if ui.selectable_label(false, &obj_path.to_string()).clicked() {
                    *visible = !*visible;
                }
            });
        }

        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.label("Log levels");
            if ui.button("Clear all").clicked() {
                self.filter_log_levels.clear();
                for v in self.filter_log_levels.values_mut() {
                    *v = false;
                }
            }
        });
        for (log_level, visible) in &mut self.filter_log_levels {
            ui.checkbox(visible, level_to_rich_text(ui, log_level));
        }
    }
}

pub(crate) fn view_filters(ui: &mut egui::Ui, state: &mut ViewTextState) -> egui::Response {
    ui.vertical(|ui| state.filters.show(ui)).response
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
    state: &ViewTextState,
    text_entries: &[TextEntry],
    scroll_to_row: Option<usize>,
) {
    // TODO: auto-resize columns

    const OBJ_PATH_HEADER: &str = "ObjPath";
    const LOG_LEVEL_HEADER: &str = "Level";
    const BODY_HEADER: &str = "Body";

    const EXTRA_SPACE_FACTOR: f32 = 1.20;

    let current_timeline = *ctx.rec_cfg.time_ctrl.timeline();
    let current_time = ctx.rec_cfg.time_ctrl.time().map(|tr| tr.floor());

    // Note: got to do all those size calculations _before_ the ui gets borrowed by the builder.

    let renderer_width = |ui: &egui::Ui, text: String| {
        let font_id = TextStyle::Body.resolve(ui.style());
        ui.fonts()
            .layout_delayed_color(text, font_id, f32::MAX)
            .size()
            .x
    };

    let timelines = state
        .filters
        .show_timelines
        .iter()
        .filter_map(|(timeline, visible)| visible.then_some(timeline))
        .map(|timeline| (timeline, renderer_width(ui, timeline.name().to_string())))
        .map(|(timeline, width)| {
            let inner_width = text_entries
                .first()
                .and_then(|te| {
                    get_time_point(ctx, te)
                        .map(|tp| tp.0.get(timeline).copied())
                        .flatten()
                })
                .map(|v| renderer_width(ui, timeline.typ().format(v)))
                .unwrap_or(f32::MIN);
            (timeline, f32::max(width, inner_width) * EXTRA_SPACE_FACTOR)
        })
        .collect::<Vec<_>>();

    let obj_path_header_size = renderer_width(ui, OBJ_PATH_HEADER.to_string());
    let obj_path_size = state
        .filters
        .filter_obj_paths
        .keys() // all of them, visible or not!
        .map(|obj_path| renderer_width(ui, obj_path.to_string()))
        .chain(std::iter::once(obj_path_header_size))
        .max_by(|a, b| a.total_cmp(&b))
        .unwrap_or(50.0)
        * EXTRA_SPACE_FACTOR;

    let log_level_header_size = renderer_width(ui, LOG_LEVEL_HEADER.to_string());
    let log_level_size = state
        .filters
        .filter_log_levels
        .keys() // all of them, visible or not!
        .map(|log_level| renderer_width(ui, log_level.to_string()))
        .chain(std::iter::once(log_level_header_size))
        .max_by(|a, b| a.total_cmp(&b))
        .unwrap_or(50.0)
        * EXTRA_SPACE_FACTOR;

    let body_header_size = renderer_width(ui, BODY_HEADER.to_string());
    let body_size = text_entries
        .iter() // all of them, visible or not!
        .map(|te| renderer_width(ui, te.body.clone())) // TODO: clone :/
        .chain(std::iter::once(body_header_size))
        .max_by(|a, b| a.total_cmp(&b))
        .unwrap_or(50.0)
        * EXTRA_SPACE_FACTOR;

    use egui_extras::Size;
    const ROW_HEIGHT: f32 = 18.0;
    const HEADER_HEIGHT: f32 = 20.0;

    let max_content_height = ui.available_height() - HEADER_HEIGHT;
    let item_spacing = ui.spacing().item_spacing;

    let resize_id = ui.id().with("__table_resize");
    ui.memory().data.remove::<Vec<f32>>(resize_id);

    let mut builder = egui_extras::TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .scroll(true);

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

        builder = builder.vertical_scroll_offset(scroll_to_offset);
    }

    let mut current_time_y = None;

    let mut builder = builder.cell_layout(egui::Layout::left_to_right(egui::Align::Center));
    {
        // timeline(s)
        for (_, size) in &timelines {
            builder = builder.column(Size::initial(*size));
        }
        // object path
        if state.filters.show_obj_path {
            builder = builder.column(Size::initial(obj_path_size));
        }
        // log level
        if state.filters.show_log_level {
            builder = builder.column(Size::initial(log_level_size));
        }
        // body
        builder = builder.column(Size::remainder().at_least(body_size));
    }
    builder
        .header(HEADER_HEIGHT, |mut header| {
            for (timeline, _) in &timelines {
                header.col(|ui| {
                    ctx.timeline_button(ui, timeline);
                });
            }
            if state.filters.show_obj_path {
                header.col(|ui| {
                    ui.strong(OBJ_PATH_HEADER);
                });
            }
            if state.filters.show_log_level {
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
                for (timeline, _) in &timelines {
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
                if state.filters.show_obj_path {
                    row.col(|ui| {
                        ctx.obj_path_button(ui, &text_entry.obj_path);
                    });
                }

                // level
                if state.filters.show_log_level {
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

    // TODO(cmc): this appears on top of the headers :/
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
