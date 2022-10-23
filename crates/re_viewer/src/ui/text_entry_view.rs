use crate::ViewerContext;
use egui::{Color32, RichText};
use egui_all::*;
use re_data_store::{InstanceProps, Objects, TextEntry};
use re_log_types::*;

// -----------------------------------------------------------------------------

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TextEntryState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// text entry window however they please when the time cursor isn't moving.
    latest_time: i64,
}

impl TextEntryState {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut ViewerContext<'_>,
        objects: &Objects<'_>,
    ) -> egui::Response {
        crate::profile_function!();

        let text_entries = collect_text_entries(ctx, objects);

        let time = ctx
            .rec_cfg
            .time_ctrl
            .time_query()
            .map_or(self.latest_time, |q| match q {
                re_data_store::TimeQuery::LatestAt(time) => time,
                re_data_store::TimeQuery::Range(range) => *range.start(),
            });

        // Did the time cursor move since last time?
        // - If it did, time to autoscroll approriately.
        // - Otherwise, let the user scroll around freely!
        let time_cursor_moved = self.latest_time != time;
        let scroll_to_row = time_cursor_moved.then(|| {
            crate::profile_scope!("binsearch");
            let index = text_entries.partition_point(|msg| msg.0.time < time);
            usize::min(index, index.saturating_sub(1))
        });

        self.latest_time = time;

        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.label(format!("{} text entries", objects.text_entry.len()));
            ui.separator();

            egui::ScrollArea::horizontal().show(ui, |ui| {
                crate::profile_scope!("render table");
                show_table(ctx, ui, &text_entries, scroll_to_row);
            })
        })
        .response
    }
}

// -----------------------------------------------------------------------------

fn collect_text_entries<'s>(
    _ctx: &mut ViewerContext<'_>,
    objects: &'s Objects<'_>,
) -> Vec<(&'s InstanceProps<'s>, &'s TextEntry<'s>)> {
    crate::profile_function!();

    let mut text_entries = {
        crate::profile_scope!("collect");

        objects.text_entry.iter().collect::<Vec<_>>()
    };

    {
        crate::profile_scope!("sort");

        text_entries.sort_by(|a, b| {
            a.0.time
                .cmp(&b.0.time)
                .then_with(|| a.0.obj_path.cmp(b.0.obj_path))
        });
    }

    text_entries
}

struct CompleteTextEntry<'s> {
    time_point: TimePoint,
    props: &'s InstanceProps<'s>,
    text_entry: &'s TextEntry<'s>,
}

impl<'s> CompleteTextEntry<'s> {
    fn try_from_props(
        ctx: &ViewerContext<'_>,
        props: &'s InstanceProps<'s>,
        text_entry: &'s TextEntry<'s>,
    ) -> Option<Self> {
        let msg = ctx.log_db.get_log_msg(props.msg_id).or_else(|| {
            re_log::warn_once!("Missing LogMsg for {:?}", props.obj_path.obj_type_path());
            None
        })?;

        let data_msg = if let LogMsg::DataMsg(data_msg) = msg {
            data_msg
        } else {
            re_log::warn_once!(
                "LogMsg must be a DataMsg ({:?})",
                props.obj_path.obj_type_path()
            );
            return None;
        };

        Some(CompleteTextEntry {
            time_point: data_msg.time_point.clone(),
            props,
            text_entry,
        })
    }
}

/// `scroll_to_row` indicates how far down we want to scroll in terms of logical rows,
/// as opposed to `scroll_to_offset` (computed below) which is how far down we want to
/// scroll in terms of actual points.
fn show_table(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text_entries: &[(&InstanceProps<'_>, &TextEntry<'_>)],
    scroll_to_row: Option<usize>,
) {
    use egui_extras::Size;
    const ROW_HEIGHT: f32 = 18.0;

    let spacing_y = ui.spacing().item_spacing.y;

    let mut builder = egui_extras::TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .scroll(true);

    if let Some(index) = scroll_to_row {
        let row_height_full = ROW_HEIGHT + spacing_y;
        let scroll_to_offset = index as f32 * row_height_full;
        builder = builder.vertical_scroll_offset(scroll_to_offset);
    }

    builder
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .columns(
            Size::initial(180.0).at_least(100.0),
            ctx.log_db.time_points.0.len(),
        ) // time(s)
        .column(Size::initial(120.0).at_least(100.0)) // path
        .column(Size::initial(60.0).at_least(60.0)) // level
        .column(Size::remainder().at_least(200.0)) // body
        .header(20.0, |mut header| {
            for timeline in ctx.log_db.time_points.0.keys() {
                header.col(|ui| {
                    ui.heading(timeline.name().as_str());
                });
            }
            header.col(|ui| {
                ui.heading("Path");
            });
            header.col(|ui| {
                ui.heading("Level");
            });
            header.col(|ui| {
                ui.heading("Body");
            });
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, text_entries.len(), |index, mut row| {
                let (props, text_entry) = text_entries[index];

                // NOTE: `try_from_props` is where we actually fetch data from the underlying
                // store, which is a costly operation.
                // Doing this here guarantees that it only happens for visible rows.
                let text_entry =
                    if let Some(te) = CompleteTextEntry::try_from_props(ctx, props, text_entry) {
                        te
                    } else {
                        row.col(|ui| {
                            ui.colored_label(
                                Color32::RED,
                                "<failed to load TextEntry from data store>",
                            );
                        });
                        return;
                    };

                let CompleteTextEntry {
                    time_point,
                    props,
                    text_entry,
                } = text_entry;

                // time(s)
                for timeline in ctx.log_db.time_points.0.keys() {
                    row.col(|ui| {
                        if let Some(value) = time_point.0.get(timeline) {
                            ctx.time_button(ui, timeline, *value);
                        }
                    });
                }

                // path
                row.col(|ui| {
                    ctx.obj_path_button(ui, props.obj_path);
                });

                // level
                row.col(|ui| {
                    if let Some(lvl) = text_entry.level {
                        ui.label(level_to_rich_text(ui, lvl));
                    } else {
                        ui.label("-");
                    }
                });

                // body
                row.col(|ui| {
                    if let Some(c) = props.color {
                        let color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                        ui.colored_label(color, text_entry.body);
                    } else {
                        ui.label(text_entry.body);
                    }
                });
            });
        });
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
