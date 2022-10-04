use crate::ViewerContext;
use nohash_hasher::IntMap;
use re_data_store::{InstanceProps, LogMessage, Objects};
use re_log_types::*;

// -----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct StateLogMessages<'s> {
    #[serde(skip)]
    objects: Objects<'s>,
}
impl<'s> StateLogMessages<'s> {
    /// Gather all `LogMessage` objects across the entire time range, not just the
    /// current time selection.
    pub fn from_context(ctx: &mut ViewerContext<'s>) -> Self {
        crate::profile_function!();

        let mut objects = re_data_store::Objects::default();
        let obj_types = ctx
            .log_db
            .obj_types
            .iter()
            .filter_map(|(obj_type_path, obj_type)| {
                matches!(&obj_type, ObjectType::LogMessage)
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

        Self { objects }
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn show(&self, ui: &mut egui::Ui, ctx: &mut ViewerContext<'_>) -> egui::Response {
        crate::profile_function!();

        let messages = self.objects.log_message.iter().collect::<Vec<_>>();
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.label(format!("{} log messages", self.objects.log_message.len()));
            ui.separator();
            log_table(ctx, ui, &messages);
        })
        .response
    }
}

fn log_table(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    messages: &[(&InstanceProps<'_>, &LogMessage<'_>)],
) {
    egui::ScrollArea::horizontal()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            use egui_extras::Size;
            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .resizable(true)
                .columns(
                    Size::initial(120.0).at_least(100.0),
                    ctx.log_db.time_points.0.len(),
                ) // time(s)
                .column(Size::initial(60.0).at_least(60.0)) // path
                .column(Size::initial(60.0).at_least(60.0)) // level
                .column(Size::remainder().at_least(180.0)) // text
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
                        ui.heading("Message");
                    });
                })
                .body(|body| {
                    const ROW_HEIGHT: f32 = 18.0;
                    body.rows(ROW_HEIGHT, messages.len(), |index, mut row| {
                        let (props, msg) = messages[index];

                        // TODO: extract time point
                        let inner = ctx.log_db.get_log_msg(props.msg_id);
                        if inner.is_none() {
                            // TODO: add error message inline? log warning? assert?
                            // TODO: crash if dev build?
                            return;
                        }

                        let inner = match inner.unwrap() {
                            LogMsg::DataMsg(inner) => inner,
                            _ => unreachable!("LogMessage must be logged as data"),
                        };

                        // time(s)
                        for time_source in ctx.log_db.time_points.0.keys() {
                            row.col(|ui| {
                                if let Some(value) = inner.time_point.0.get(time_source) {
                                    ctx.time_button(ui, time_source, *value);
                                }
                            });
                        }

                        // path
                        row.col(|ui| {
                            // ctx.data_path_button(ui, data_path)
                            ui.label(props.obj_path.to_string());
                        });

                        // level
                        row.col(|ui| {
                            ui.label(
                                msg.level
                                    .map_or_else(|| "-".to_owned(), |lvl| lvl.to_owned()),
                            );
                        });

                        // text
                        row.col(|ui| {
                            ui.label(msg.text.to_owned());
                        });
                    });
                });
        });
}
