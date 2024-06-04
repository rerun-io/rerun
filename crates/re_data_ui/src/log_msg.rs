use re_log_types::{BlueprintActivationCommand, LogMsg, SetStoreInfo, StoreInfo};
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;

impl DataUi for LogMsg {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store2::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        match self {
            Self::SetStoreInfo(msg) => msg.data_ui(ctx, ui, ui_layout, query, db),
            Self::ArrowMsg(_, _) => {}
            Self::BlueprintActivationCommand(BlueprintActivationCommand {
                blueprint_id,
                make_active,
                make_default,
            }) => {
                ui.label(format!(
                    "BlueprintActivationCommand({blueprint_id}, make_active: {make_active}, make_default: {make_default})"
                ));
            }
        }
    }
}

impl DataUi for SetStoreInfo {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _ui_layout: UiLayout,
        _query: &re_data_store2::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        let Self { row_id: _, info } = self;
        let StoreInfo {
            application_id,
            store_id,
            cloned_from,
            started,
            store_source,
            is_official_example,
            store_version,
        } = info;

        ui.code("SetStoreInfo");

        egui::Grid::new("fields").num_columns(2).show(ui, |ui| {
            ui.grid_left_hand_label("application_id:");
            ui.label(application_id.to_string());
            ui.end_row();

            ui.grid_left_hand_label("store_id:");
            ui.label(format!("{store_id:?}"));
            ui.end_row();

            ui.grid_left_hand_label("cloned_from");
            if let Some(cloned_from) = cloned_from {
                crate::item_ui::store_id_button_ui(ctx, ui, cloned_from);
            }
            ui.end_row();

            ui.grid_left_hand_label("started:");
            ui.label(started.format(ctx.app_options.time_zone));
            ui.end_row();

            ui.grid_left_hand_label("store_source:");
            ui.label(format!("{store_source}"));
            ui.end_row();

            if let Some(store_version) = store_version {
                ui.grid_left_hand_label("store_version:");
                ui.label(format!("{store_version}"));
                ui.end_row();
            } else {
                re_log::debug_once!("store version is undefined for this recording, this is a bug");
            }

            ui.grid_left_hand_label("is_official_example:");
            ui.label(format!("{is_official_example}"));
            ui.end_row();

            ui.grid_left_hand_label("store_kind:");
            ui.label(format!("{}", store_id.kind));
            ui.end_row();
        });
    }
}
