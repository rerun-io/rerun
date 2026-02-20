use itertools::Itertools as _;
use re_entity_db::EntityDb;
use re_log_types::ApplicationId;
use re_sdk_types::archetypes::RecordingInfo;
use re_sdk_types::components::Timestamp;
use re_viewer_context::{UiLayout, ViewerContext};

use crate::item_ui::entity_db_button_ui;

impl crate::DataUi for ApplicationId {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        _query: &re_chunk_store::LatestAtQuery,
        _db: &re_entity_db::EntityDb,
    ) {
        egui::Grid::new("application_id")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Application ID");

                let mut label = self.to_string();
                if self == ctx.store_context.application_id() {
                    label.push_str(" (active)");
                }
                UiLayout::List.label(ui, label);
                ui.end_row();
            });

        // Find all recordings with this app id
        let recordings: Vec<&EntityDb> = ctx
            .store_bundle()
            .recordings()
            .filter(|db| db.application_id() == self)
            .sorted_by_key(|entity_db| {
                entity_db.recording_info_property::<Timestamp>(
                    RecordingInfo::descriptor_start_time().component,
                )
            })
            .collect();

        match ui_layout {
            UiLayout::List => {
                // Too little space for anything else
            }
            UiLayout::Tooltip => {
                if recordings.len() == 1 {
                    ui.label("There is 1 loaded recording for this app.");
                } else {
                    ui.label(format!(
                        "There are {} loaded recordings for this app.",
                        re_format::format_uint(recordings.len()),
                    ));
                }
            }
            UiLayout::SelectionPanel => {
                if !recordings.is_empty() {
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;

                        ui.add_space(8.0);
                        ui.strong("Loaded recordings for this app");
                        for entity_db in recordings {
                            entity_db_button_ui(ctx, ui, entity_db, ui_layout, true);
                        }
                    });
                }
            }
        }
    }
}
