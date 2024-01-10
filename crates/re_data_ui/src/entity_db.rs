impl crate::DataUi for re_entity_db::EntityDb {
    fn data_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: re_viewer_context::UiVerbosity,
        _query: &re_data_store::LatestAtQuery,
    ) {
        let re_ui = &ctx.re_ui;

        if verbosity == re_viewer_context::UiVerbosity::Small {
            let mut string = self.store_id().to_string();
            if let Some(data_source) = &self.data_source {
                string += &format!(", {data_source}");
            }
            if let Some(store_info) = self.store_info() {
                string += &format!(", {}", store_info.application_id);
            }
            ui.label(string);
            return;
        }

        egui::Grid::new("entity_db").num_columns(2).show(ui, |ui| {
            re_ui.grid_left_hand_label(ui, &format!("{} ID", self.store_id().kind));
            ui.label(self.store_id().to_string());
            ui.end_row();

            if let Some(data_source) = &self.data_source {
                re_ui.grid_left_hand_label(ui, "Data source");
                ui.label(data_source.to_string());
                ui.end_row();
            }

            if let Some(store_info) = self.store_info() {
                let re_log_types::StoreInfo {
                    application_id,
                    store_id: _,
                    is_official_example: _,
                    started,
                    store_source,
                    store_kind,
                } = store_info;

                re_ui.grid_left_hand_label(ui, "Application ID");
                ui.label(application_id.to_string());
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Recording started");
                ui.label(started.format(ctx.app_options.time_zone_for_timestamps));
                ui.end_row();

                re_ui.grid_left_hand_label(ui, "Source");
                ui.label(store_source.to_string());
                ui.end_row();

                // We are in the recordings menu, we know the kind
                if false {
                    re_ui.grid_left_hand_label(ui, "Kind");
                    ui.label(store_kind.to_string());
                    ui.end_row();
                }
            }
        });
    }
}
