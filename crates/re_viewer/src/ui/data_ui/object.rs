use re_data_store::{Index, InstanceId, ObjPath};
use re_log_types::{
    field_types::{ClassId, KeypointId},
    Data, DataPath,
};
use re_query::{get_component_with_instances, QueryError};

use crate::{
    misc::ViewerContext,
    ui::{annotations::AnnotationMap, Preview},
};

use super::{
    component::{arrow_component_elem_ui, arrow_component_ui},
    DataUi,
};

impl DataUi for ObjPath {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        InstanceId {
            obj_path: self.clone(),
            instance_index: None,
        }
        .data_ui(ctx, ui, preview)
    }
}

impl DataUi for InstanceId {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        preview: Preview,
    ) -> egui::Response {
        let timeline = ctx.rec_cfg.time_ctrl.timeline();
        if ctx
            .log_db
            .obj_db
            .arrow_store
            .all_components(timeline, &self.obj_path)
            .is_some()
        {
            generic_arrow_ui(ctx, ui, self, preview)
        } else {
            generic_instance_ui(ctx, ui, self, preview)
        }
    }
}

fn generic_arrow_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_id: &InstanceId,
    _preview: Preview,
) -> egui::Response {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let store = &ctx.log_db.obj_db.arrow_store;

    let Some(time) = ctx.rec_cfg.time_ctrl.time_int() else {
        return ui.label(ctx.re_ui.error_text("No current time."))
    };

    let query = re_arrow_store::LatestAtQuery::new(*timeline, time);

    let Some(components) = store.all_components(timeline, &instance_id.obj_path)
    else {
        return ui.label("No Components")
    };

    egui::Grid::new("entity_instance")
        .num_columns(2)
        .show(ui, |ui| {
            for component in components {
                let component_data =
                    get_component_with_instances(store, &query, &instance_id.obj_path, component);

                ctx.data_path_button_to(
                    ui,
                    component.to_string(),
                    &DataPath::new_arrow(instance_id.obj_path.clone(), component),
                );

                match (component_data, &instance_id.instance_index) {
                    // If we didn't find the component then it's not set at this point in time
                    (Err(QueryError::PrimaryNotFound), _) => ui.label("<unset>"),
                    // Any other failure to get a component is unexpected
                    (Err(err), _) => ui.label(format!("Error: {}", err)),
                    // If an `instance_index` wasn't provided, just report the number of values
                    (Ok(component_data), None) => {
                        arrow_component_ui(ctx, ui, &component_data, Preview::Small)
                    }
                    // If the `instance_index` is an `ArrowInstance` show the value
                    (Ok(component_data), Some(Index::ArrowInstance(instance))) => {
                        arrow_component_elem_ui(ctx, ui, &component_data, instance, Preview::Small)
                    }
                    // If the `instance_index` isn't an `ArrowInstance` something has gone wrong
                    // TODO(jleibs) this goes away once all indexes are just `Instances`
                    (Ok(_), Some(_)) => ui.label("<bad index>"),
                };

                ui.end_row();
            }
            Some(())
        })
        .response
}

fn generic_instance_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    instance_id: &InstanceId,
    preview: Preview,
) -> egui::Response {
    let timeline = ctx.rec_cfg.time_ctrl.timeline();
    let Some(store) = ctx.log_db.obj_db.store.get(timeline) else {
        return ui.label(ctx.re_ui.error_text("No store with timeline {timeline}."))
    };
    let Some(time_i64) = ctx.rec_cfg.time_ctrl.time_i64() else {
        return ui.label(ctx.re_ui.error_text("No current time."))
    };
    let time_query = re_data_store::TimeQuery::LatestAt(time_i64);
    let Some(obj_store) = store.get(&instance_id.obj_path) else {
        return ui.label(ctx.re_ui.error_text(format!("No object at path {}", instance_id.obj_path)))
    };

    let mut class_id = None;
    let mut keypoint_id = None;

    let grid_resp = egui::Grid::new("object_instance")
        .num_columns(2)
        .show(ui, |ui| {
            for (field_name, field_store) in obj_store.iter() {
                ctx.data_path_button_to(
                    ui,
                    field_name.to_string(),
                    &DataPath::new(instance_id.obj_path.clone(), *field_name),
                );

                match field_store
                    .query_field_to_datavec(&time_query, instance_id.instance_index.as_ref())
                {
                    Ok((_, data_vec)) => {
                        if data_vec.len() == 1 {
                            let data = data_vec.last().unwrap();
                            if field_name.as_str() == "class_id" {
                                if let Data::I32(id) = data {
                                    class_id = Some(ClassId(id as _));
                                }
                            }
                            if field_name.as_str() == "keypoint_id" {
                                if let Data::I32(id) = data {
                                    keypoint_id = Some(KeypointId(id as _));
                                }
                            }
                            data.data_ui(ctx, ui, preview);
                        } else {
                            data_vec.data_ui(ctx, ui, preview);
                        }
                    }
                    Err(err) => {
                        re_log::warn_once!("Bad data for {instance_id}: {err}");
                        ui.label(ctx.re_ui.error_text(format!("Data error: {:?}", err)));
                    }
                }

                ui.end_row();
            }
        })
        .response;

    // If we have a class id, show some information about the resolved style!
    if let Some(class_id) = class_id {
        ui.separator();

        let resp = if let Some((data_path, annotations)) =
            AnnotationMap::find_associated(ctx, instance_id.obj_path.clone())
        {
            ctx.data_path_button_to(
                ui,
                format!("Annotation Context at {}", data_path.obj_path),
                &data_path,
            );
            egui::Grid::new("class_description")
                .num_columns(2)
                .show(ui, |ui| {
                    if let Some(class_description) = annotations.context.class_map.get(&class_id) {
                        let class_annotation = &class_description.info;
                        let mut keypoint_annotation = None;

                        if let Some(keypoint_id) = keypoint_id {
                            keypoint_annotation = class_description.keypoint_map.get(&keypoint_id);
                            if keypoint_annotation.is_none() {
                                ui.label(ctx.re_ui.warning_text(format!(
                                    "unknown keypoint_id {}",
                                    keypoint_id.0
                                )));
                            }
                        }

                        if let Some(label) = keypoint_annotation
                            .and_then(|a| a.label.as_ref())
                            .or(class_annotation.label.as_ref())
                        {
                            ui.label("label");
                            ui.label(label.0.as_str());
                            ui.end_row();
                        }
                        if let Some(color) = keypoint_annotation
                            .and_then(|a| a.color.as_ref())
                            .or(class_annotation.color.as_ref())
                        {
                            ui.label("color");
                            color.data_ui(ctx, ui, preview);
                            ui.end_row();
                        }
                    } else {
                        ui.label(
                            ctx.re_ui
                                .warning_text(format!("unknown class_id {}", class_id.0)),
                        );
                    }
                })
                .response
        } else {
            ui.label(
                ctx.re_ui
                    .warning_text("class_id specified, but no annotation context found"),
            )
        };

        resp.union(grid_resp)
    } else {
        grid_resp
    }
}
