use re_log_types::DataPath;

use super::DataUi;

/// Previously `data_path_ui()`
impl DataUi for DataPath {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: crate::ui::Preview,
    ) -> egui::Response {
        if self.is_arrow() {
            ui.label("TODO(jleibs): DataPath query for Arrow")
        } else {
            let timeline = ctx.rec_cfg.time_ctrl.timeline();

            if let Some(time_i64) = ctx.rec_cfg.time_ctrl.time_i64() {
                let time_query = re_data_store::TimeQuery::LatestAt(time_i64);

                match ctx
                    .log_db
                    .obj_db
                    .store
                    .query_data_path(timeline, &time_query, self)
                {
                    Some(Ok((_, data_vec))) => {
                        if data_vec.len() == 1 {
                            let data = data_vec.last().unwrap();
                            data.detailed_data_ui(ctx, ui, preview)
                        } else {
                            data_vec.data_ui(ctx, ui, preview)
                        }
                    }
                    Some(Err(err)) => {
                        re_log::warn_once!("Bad data for {self}: {err}");
                        ui.label(ctx.re_ui.error_text(format!("Data error: {:?}", err)))
                    }
                    None => ui.label(ctx.re_ui.error_text(format!("No data at time {time_i64}"))),
                }
            } else {
                ui.label(ctx.re_ui.error_text("No current time."))
            }
        }
    }
}
