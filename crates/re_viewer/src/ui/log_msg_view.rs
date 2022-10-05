use crate::ViewerContext;
use egui::Color32;
use nohash_hasher::IntMap;
use re_data_store::{InstanceProps, Objects, TextEntry};
use re_log_types::*;

// -----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct TextEntryFetcher<'s> {
    #[serde(skip)]
    objects: Objects<'s>,
}
impl<'s> TextEntryFetcher<'s> {
    /// Gather all `LogMessage` objects across the entire time range, not just the
    /// current time selection.
    pub fn from_context(ctx: &mut ViewerContext<'s>, space: Option<&ObjPath>) -> Self {
        crate::profile_function!();

        let mut objects = re_data_store::Objects::default();
        let obj_types = ctx
            .log_db
            .obj_types
            .iter()
            .filter_map(|(obj_type_path, obj_type)| {
                matches!(&obj_type, ObjectType::TextEntry)
                    .then(|| (obj_type_path.clone(), *obj_type))
            })
            .collect::<IntMap<_, _>>();

        let time_source = ctx.rec_cfg.time_ctrl.source();
        let all_time = re_data_store::TimeQuery::<i64>::Range(i64::MIN..=i64::MAX);

        if let Some(store) = ctx.log_db.data_store.get(time_source) {
            for (obj_path, obj_store) in store.iter() {
                if let Some(obj_type) = obj_types.get(obj_path.obj_type_path()) {
                    objects.query_object(obj_store, &all_time, obj_path, obj_type);
                }
            }
        }

        let objects = objects.filter(|props| {
            props.space == space
                && ctx
                    .rec_cfg
                    .projected_object_properties
                    .get(props.obj_path)
                    .visible
        });

        Self { objects }
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn show(&self, ui: &mut egui::Ui, ctx: &mut ViewerContext<'s>) -> egui::Response {
        crate::profile_function!();

        let text_entries = collect_text_entries(ctx, &self.objects);

        // TODO(cmc): There are some rendering issues with horizontal scrolling here that seem
        // to stem from the interaction between egui's Table and the docking system.
        // Specifically, the text from the remainder column is incorrectly clipped.
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.label(format!("{} log messages", self.objects.text_entry.len()));
            ui.separator();
            show_table(ctx, ui, &text_entries);
        })
        .response
    }
}

// -----------------------------------------------------------------------------

struct CompleteTextEntry<'s> {
    data_path: DataPath,
    time_point: TimePoint,
    time: TimeInt,
    props: &'s InstanceProps<'s>,
    text_entry: &'s TextEntry<'s>,
}

fn collect_text_entries<'a, 's>(
    ctx: &mut ViewerContext<'s>,
    objects: &'a Objects<'s>,
) -> Vec<CompleteTextEntry<'a>> {
    let time_source = ctx.rec_cfg.time_ctrl.source();
    let mut text_entries = objects
        .text_entry
        .iter()
        .filter_map(|(props, text_entry)| {
            // TODO(cmc): let-else cannot land soon enough...

            let msg = if let Some(msg) = ctx.log_db.get_log_msg(props.msg_id) {
                msg
            } else {
                re_log::warn_once!("Missing LogMsg for {:?}", props.obj_path.obj_type_path());
                return None;
            };

            let data_msg = if let LogMsg::DataMsg(data_msg) = msg {
                data_msg
            } else {
                re_log::warn_once!(
                    "LogMsg must be a DataMsg ({:?})",
                    props.obj_path.obj_type_path()
                );
                return None;
            };

            let time = data_msg
                .time_point
                .0
                .get(time_source)
                .map_or(TimeInt::BEGINNING, |t| *t);

            Some(CompleteTextEntry {
                data_path: data_msg.data_path.clone(),
                time_point: data_msg.time_point.clone(),
                time,
                props,
                text_entry,
            })
        })
        .collect::<Vec<_>>();

    // First sort along the time axis, then along paths.
    text_entries.sort_by(|a, b| {
        a.time
            .cmp(&b.time)
            .then_with(|| a.data_path.obj_path().cmp(b.data_path.obj_path()))
    });

    text_entries
}

fn show_table(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, messages: &[CompleteTextEntry<'_>]) {
    use egui_extras::Size;
    egui_extras::TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .scroll(false)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .columns(
            Size::initial(120.0).at_least(100.0),
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
            const ROW_HEIGHT: f32 = 18.0;
            body.rows(ROW_HEIGHT, messages.len(), |index, mut row| {
                let CompleteTextEntry {
                    time_point,
                    data_path,
                    time: _,
                    props,
                    text_entry,
                } = &messages[index];

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
                        ui.colored_label(level_to_color(ui, lvl), lvl);
                    } else {
                        ui.label("-");
                    }
                });

                // body
                row.col(|ui| {
                    if let Some(c) = props.color {
                        let color = Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]);
                        ui.colored_label(color, text_entry.body);
                    } else {
                        ui.label(text_entry.body);
                    }
                });
            });
        });
}

fn level_to_color(ui: &egui::Ui, lvl: &str) -> Color32 {
    match lvl {
        "TRACE" => Color32::LIGHT_GRAY,
        "DEBUG" => Color32::LIGHT_BLUE,
        "INFO" => Color32::LIGHT_GREEN,
        "WARN" => Color32::YELLOW,
        "ERROR" => Color32::RED,
        _ => ui.visuals().text_color(),
    }
}
