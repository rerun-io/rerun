use re_log_types::{DataPath, FieldOrComponent};

use super::{component::arrow_component_ui, DataUi};

impl DataUi for DataPath {
    fn data_ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: crate::ui::UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.horizontal(|ui| {
            ui.label("Object path:");
            ctx.obj_path_button(ui, None, &self.obj_path);
        });

        if let FieldOrComponent::Component(component) = self.field_name {
            let store = &ctx.log_db.obj_db.arrow_store;

            match re_query::get_component_with_instances(store, query, self.obj_path(), component) {
                Err(re_query::QueryError::PrimaryNotFound) => {
                    ui.label("<unset>");
                }
                // Any other failure to get a component is unexpected
                Err(err) => {
                    ui.label(format!("Error: {}", err));
                }
                Ok(component_data) => {
                    arrow_component_ui(ctx, ui, &component_data, verbosity, query);
                }
            }
        } else {
            let time_query = re_data_store::TimeQuery::LatestAt(query.at.as_i64());

            match ctx
                .log_db
                .obj_db
                .store
                .query_data_path(&query.timeline, &time_query, self)
            {
                Some(Ok((_, data_vec))) => {
                    if data_vec.len() == 1 {
                        let data = data_vec.last().unwrap();
                        data.data_ui(ctx, ui, verbosity, query);
                    } else {
                        data_vec.data_ui(ctx, ui, verbosity, query);
                    }
                }
                Some(Err(err)) => {
                    re_log::warn_once!("Bad data for {self}: {err}");
                    ui.label(ctx.re_ui.error_text(format!("Data error: {:?}", err)));
                }
                None => {
                    ui.label(format!(
                        "No data at {}",
                        query.timeline.typ().format(query.at)
                    ));
                }
            }
        }
    }
}
