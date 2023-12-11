use crate::view_part_system::EmptySystem;
use egui_extras::Column;
use itertools::Itertools;
use re_data_store::{EntityProperties, InstancePath};
use re_log_types::EntityPath;
use re_query::get_component_with_instances;
use re_viewer_context::{
    AutoSpawnHeuristic, PerSystemEntities, SpaceViewClass, SpaceViewClassRegistryError,
    SpaceViewId, SpaceViewSystemExecutionError, UiVerbosity, ViewContextCollection,
    ViewPartCollection, ViewQuery, ViewerContext,
};

#[derive(Default)]
pub struct DataSpaceView;

impl SpaceViewClass for DataSpaceView {
    type State = ();

    const IDENTIFIER: &'static str = "Data";
    const DISPLAY_NAME: &'static str = "Data";

    fn icon(&self) -> &'static re_ui::Icon {
        //TODO(ab): fix that icon
        &re_ui::icons::SPACE_VIEW_TEXTBOX
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi) -> egui::WidgetText {
        "Show the data contained in entities in a table.".into()
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError> {
        system_registry.register_part_system::<EmptySystem>()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::Low
    }

    fn auto_spawn_heuristic(
        &self,
        _ctx: &ViewerContext<'_>,
        _space_origin: &EntityPath,
        _ent_paths: &PerSystemEntities,
    ) -> re_viewer_context::AutoSpawnHeuristic {
        AutoSpawnHeuristic::NeverSpawn
    }

    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        _view_ctx: &ViewContextCollection,
        _parts: &ViewPartCollection,
        query: &ViewQuery<'_>,
        _draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let entities: Vec<_> = query
            .iter_all_data_results()
            .filter(|data_result| data_result.resolved_properties.visible)
            .map(|data_result| &data_result.entity_path)
            .unique()
            .cloned()
            .collect();

        let store = ctx.store_db.store();
        let latest_at_query = query.latest_at_query();

        // for each entity, this does the union of all instance keys of all components
        let all_instances: Vec<_> = entities
            .iter()
            .flat_map(|entity| {
                store
                    .all_components(&query.timeline, entity)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|comp| !comp.is_indicator_component())
                    .flat_map(|comp| {
                        get_component_with_instances(store, &latest_at_query, entity, comp)
                            .map(|(_, comp_inst)| comp_inst.instance_keys())
                            .unwrap_or_default()
                    })
                    .filter(|instance_key| !instance_key.is_splat())
                    .map(|instance_key| InstancePath::instance(entity.clone(), instance_key))
            })
            .unique()
            .collect();

        let all_components: Vec<_> = entities
            .iter()
            .flat_map(|entity| {
                store
                    .all_components(&query.timeline, entity)
                    .unwrap_or_default()
            })
            .unique()
            .filter(|comp| !comp.is_indicator_component())
            .collect();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame {
                    inner_margin: egui::Margin::same(5.0),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    egui_extras::TableBuilder::new(ui)
                        .columns(
                            Column::auto_with_initial_suggestion(200.0),
                            all_components.len() + 1,
                        )
                        .resizable(true)
                        .vscroll(false)
                        .auto_shrink([false, true])
                        .striped(true)
                        .header(re_ui::ReUi::table_line_height(), |mut row| {
                            row.col(|ui| {
                                ui.strong("Entity");
                            });

                            for comp in &all_components {
                                row.col(|ui| {
                                    ui.strong(comp.short_name());
                                });
                            }
                        })
                        .body(|body| {
                            body.rows(
                                re_ui::ReUi::table_line_height(),
                                all_instances.len(),
                                |idx, mut row| {
                                    let instance = &all_instances[idx];

                                    row.col(|ui| {
                                        ui.label(format!("{instance}"));
                                    });

                                    for comp in &all_components {
                                        row.col(|ui| {
                                            if let Some((_, comp_inst)) =
                                                get_component_with_instances(
                                                    store,
                                                    &latest_at_query,
                                                    &instance.entity_path,
                                                    *comp,
                                                )
                                            {
                                                ctx.component_ui_registry.ui(
                                                    ctx,
                                                    ui,
                                                    UiVerbosity::Small,
                                                    &latest_at_query,
                                                    &instance.entity_path,
                                                    &comp_inst,
                                                    &instance.instance_key,
                                                );
                                            } else {
                                                ui.weak("-");
                                            }
                                        });
                                    }
                                },
                            );
                        });
                });
            });

        Ok(())
    }
}
