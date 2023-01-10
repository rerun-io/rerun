use re_log_types::{DataPath, FieldOrComponent};

use super::{component::arrow_component_ui, DataUi};

/// Previously `data_path_ui()`
impl DataUi for DataPath {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: crate::ui::Preview,
    ) -> egui::Response {
        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        if let Some(time_i64) = ctx.rec_cfg.time_ctrl.time_i64() {
            if let FieldOrComponent::Component(component) = self.field_name {
                let store = &ctx.log_db.obj_db.arrow_store;
                let query = re_arrow_store::LatestAtQuery::new(*timeline, time_i64.into());

                match re_query::get_component_with_instances(
                    store,
                    &query,
                    self.obj_path(),
                    component,
                ) {
                    Err(re_query::QueryError::PrimaryNotFound) => ui.label("<unset>"),
                    // Any other failure to get a component is unexpected
                    Err(err) => ui.label(format!("Error: {}", err)),
                    Ok(component_data) => arrow_component_ui(ctx, ui, &component_data, preview),
                }
            } else {
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
            }
        } else {
            ui.label(ctx.re_ui.error_text("No current time."))
        }
    }
}
