use std::sync::Arc;

use re_entity_db::InstancePath;
use re_log_types::ComponentPath;
use re_viewer_context::{UiLayout, ViewerContext};

use super::DataUi;
use crate::item_ui;

impl DataUi for InstancePath {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_data_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            entity_path,
            instance,
        } = self;

        let Some(components) = ctx
            .recording_store()
            .all_components(&query.timeline(), entity_path)
        else {
            if ctx.recording().is_known_entity(entity_path) {
                // This is fine - e.g. we're looking at `/world` and the user has only logged to `/world/car`.
                ui.label(format!(
                    "No components logged on timeline {:?}",
                    query.timeline().name()
                ));
            } else {
                ui.label(
                    ctx.re_ui
                        .error_text(format!("Unknown entity: {entity_path:?}")),
                );
            }
            return;
        };

        let mut components = crate::component_list_for_ui(&components);

        let split = components.partition_point(|c| c.is_indicator_component());
        let normal_components = components.split_off(split);
        let indicator_components = components;

        let show_indicator_comps = match ui_layout {
            UiLayout::List | UiLayout::Tooltip => {
                // Skip indicator components in hover ui (unless there are no other
                // types of components).
                !normal_components.is_empty()
            }
            UiLayout::SelectionPanelLimitHeight | UiLayout::SelectionPanelFull => true,
        };

        // First show indicator components, outside the grid:
        if show_indicator_comps {
            for component_name in indicator_components {
                item_ui::component_path_button(
                    ctx,
                    ui,
                    &ComponentPath::new(entity_path.clone(), component_name),
                    db,
                );
            }
        }

        // Now show the rest of the components:
        egui::Grid::new("components")
            .spacing(ui.spacing().item_spacing)
            .num_columns(2)
            .show(ui, |ui| {
                for component_name in normal_components {
                    let results = db.query_caches().latest_at(
                        db.store(),
                        query,
                        entity_path,
                        [component_name],
                    );
                    let Some(results) = results.components.get(&component_name) else {
                        continue; // no need to show components that are unset at this point in time
                    };

                    item_ui::component_path_button(
                        ctx,
                        ui,
                        &ComponentPath::new(entity_path.clone(), component_name),
                        db,
                    );

                    if instance.is_all() {
                        crate::EntityLatestAtResults {
                            entity_path: entity_path.clone(),
                            component_name,
                            results: Arc::clone(results),
                        }
                        .data_ui(ctx, ui, UiLayout::List, query, db);
                    } else {
                        ctx.component_ui_registry.ui(
                            ctx,
                            ui,
                            UiLayout::List,
                            query,
                            db,
                            entity_path,
                            results,
                            instance,
                        );
                    }

                    ui.end_row();
                }
                Some(())
            });
    }
}
