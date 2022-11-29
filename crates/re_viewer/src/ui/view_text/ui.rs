use egui::{Color32, NumExt as _, RichText};

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
}

pub(crate) fn view_text(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewTextState,
    scene: &SceneText,
) -> egui::Response {
    crate::profile_function!();

    let time = ctx
        .rec_cfg
        .time_ctrl
        .time_i64()
        .unwrap_or(state.latest_time);

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
            show_table(ctx, ui, &scene.text_entries, scroll_to_row);
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
    text_entries: &[TextEntry],
    scroll_to_row: Option<usize>,
) {
    use egui_extras::Size;
    const ROW_HEIGHT: f32 = 18.0;
    const HEADER_HEIGHT: f32 = 20.0;

    let max_content_height = ui.available_height() - HEADER_HEIGHT;
    let item_spacing = ui.spacing().item_spacing;

    let current_timeline = *ctx.rec_cfg.time_ctrl.timeline();
    let current_time = ctx.rec_cfg.time_ctrl.time_int();

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

    builder
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .columns(
            Size::initial(140.0).at_least(50.0), // timelines
            ctx.log_db.timelines().count(),
        ) // time(s)
        .column(Size::initial(120.0).at_least(50.0)) // path
        .column(Size::initial(50.0).at_least(50.0)) // level
        .column(Size::remainder().at_least(200.0)) // body
        .header(HEADER_HEIGHT, |mut header| {
            for timeline in ctx.log_db.timelines() {
                header.col(|ui| {
                    ctx.timeline_button(ui, timeline);
                });
            }
            header.col(|ui| {
                ui.strong("Path");
            });
            header.col(|ui| {
                ui.strong("Level");
            });
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

                // time(s)
                for timeline in ctx.log_db.timelines() {
                    row.col(|ui| {
                        if let Some(value) = time_point.0.get(timeline).copied() {
                            if let Some(current_time) = current_time {
                                if current_time_y.is_none()
                                    && timeline == &current_timeline
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
                row.col(|ui| {
                    ctx.obj_path_button(ui, &text_entry.obj_path);
                });

                // level
                row.col(|ui| {
                    if let Some(lvl) = &text_entry.level {
                        ui.label(level_to_rich_text(ui, lvl));
                    } else {
                        ui.label("-");
                    }
                });

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
