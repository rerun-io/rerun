use crate::ViewerContext;
use egui::{Color32, RichText};
use re_data_store::{InstanceProps, Objects, TextEntry};
use re_log_types::*;

// -----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TextEntryState {
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
            .map(|q| match q {
                re_data_store::TimeQuery::LatestAt(time) => time,
                re_data_store::TimeQuery::Range(range) => *range.start(),
            })
            .expect("there is always an active time query");

        let index = (self.latest_time != time).then(|| {
            crate::profile_scope!("binsearch");
            match text_entries.binary_search_by(|msg| msg.time.cmp(&time)) {
                Ok(i) => i,
                Err(i) => usize::min(i, text_entries.len() - 1),
            }
        });

        self.latest_time = time;

        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.label(format!("{} text entries", objects.text_entry.len()));
            ui.separator();

            egui::ScrollArea::horizontal().show(ui, |ui| {
                crate::profile_scope!("render table");
                show_table(ctx, ui, &text_entries, index);
            })
        })
        .response
    }
}

// -----------------------------------------------------------------------------

struct CompleteTextEntry<'s> {
    data_path: DataPath,
    time_point: TimePoint,
    time: i64,
    props: &'s InstanceProps<'s>,
    text_entry: &'s TextEntry<'s>,
}

fn collect_text_entries<'s>(
    ctx: &mut ViewerContext<'_>,
    objects: &'s Objects<'_>,
) -> Vec<CompleteTextEntry<'s>> {
    crate::profile_function!();

    let mut text_entries = {
        crate::profile_scope!("collect");

        objects
            .text_entry
            .iter()
            .filter_map(|(props, text_entry)| {
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
                    data_path: data_msg.data_path.clone(),
                    time_point: data_msg.time_point.clone(),
                    time: props.time,
                    props,
                    text_entry,
                })
            })
            .collect::<Vec<_>>()
    };

    {
        crate::profile_scope!("sort");

        // First sort along the time axis, then along paths.
        text_entries.sort_by(|a, b| {
            a.time
                .cmp(&b.time)
                .then_with(|| a.data_path.obj_path().cmp(b.data_path.obj_path()))
        });
    }

    text_entries
}

fn show_table(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    text_entries: &[CompleteTextEntry<'_>],
    index: Option<usize>,
) {
    use egui_extras::Size;
    const ROW_HEIGHT: f32 = 18.0;

    let spacing_y = ui.spacing().item_spacing.y;

    let mut builder = egui_extras::TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .scroll(true);

    if let Some(index) = index {
        let row_height_full = ROW_HEIGHT + spacing_y;
        let offset = index as f32 * row_height_full;

        builder = builder.vertical_scroll_offset(offset);
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
            for time_source in ctx.log_db.time_points.0.keys() {
                header.col(|ui| {
                    ui.heading(time_source.name().as_str());
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
                let CompleteTextEntry {
                    time_point,
                    data_path,
                    time: _,
                    props,
                    text_entry,
                } = &text_entries[index];

                // time(s)
                for time_source in ctx.log_db.time_points.0.keys() {
                    row.col(|ui| {
                        if let Some(value) = time_point.0.get(time_source) {
                            ctx.time_button(ui, time_source, *value);
                        }
                    });
                }

                // path
                row.col(|ui| {
                    ctx.obj_path_button(ui, data_path.obj_path());
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
