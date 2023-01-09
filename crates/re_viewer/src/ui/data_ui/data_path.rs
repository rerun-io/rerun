use re_data_store::ObjPath;
use re_log_types::{
    external::arrow2::array, msg_bundle::Component, AnnotationContext, ComponentName, DataPath,
    FieldOrComponent,
};

use super::DataUi;

/// Previously `data_path_ui()`
impl DataUi for DataPath {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: crate::ui::Preview,
    ) -> egui::Response {
        if let FieldOrComponent::Component(component) = self.field_name {
            if component == AnnotationContext::name() {
                ui.label("TODO: Annotation context")
            } else {
                generic_arrow_component_ui(ctx, ui, &self.obj_path, &component, preview)
            }
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

fn generic_arrow_component_ui(
    ctx: &mut crate::misc::ViewerContext<'_>,
    ui: &mut egui::Ui,
    ent_path: &ObjPath,
    component: &ComponentName,
    _preview: crate::ui::Preview,
) -> egui::Response {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let store = &ctx.log_db.obj_db.arrow_store;

    let Some(time) = ctx.rec_cfg.time_ctrl.time_int() else {
        return ui.label(ctx.re_ui.error_text("No current time."))
    };

    let query = re_arrow_store::LatestAtQuery::new(*timeline, time);

    match re_query::get_component_with_instances(store, &query, ent_path, *component) {
        Err(re_query::QueryError::PrimaryNotFound) => ui.label("<unset>"),
        // Any other failure to get a component is unexpected
        Err(err) => ui.label(format!("Error: {}", err)),
        Ok(data) => match data.iter_instance_keys() {
            Ok(instance_keys) => {
                match data.len() {
                    0 => ui.label("empty"),
                    1..=100 => {
                        egui::Grid::new("component")
                            .num_columns(2)
                            .show(ui, |ui| {
                                for instance in instance_keys {
                                    ui.label(format!("{}", instance));
                                    if let Some(value) = data.lookup(&instance) {
                                        // TODO(jleibs): Dispatch to prettier printers for
                                        // component types we know about.
                                        let mut repr = String::new();
                                        let display = array::get_display(value.as_ref(), "null");
                                        display(&mut repr, 0).unwrap();
                                        ui.label(repr);
                                    } else {
                                        ui.label("<unset>");
                                    }
                                    ui.end_row();
                                }
                            })
                            .response
                    }
                    num => ui.label(format!("{} values", num)),
                }
            }
            Err(err) => ui.label(format!("Error: {}", err)),
        },
    }
}
