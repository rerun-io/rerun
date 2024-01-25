use std::collections::BTreeSet;

use itertools::Itertools;
use re_data_store::{DataStore, LatestAtQuery};
use re_data_ui::{is_component_visible_in_ui, item_ui, temporary_style_ui_for_component};
use re_entity_db::InstancePath;
use re_log_types::{ComponentPath, DataCell, DataRow, RowId, StoreKind};
use re_query_cache::external::re_query::get_component_with_instances;
use re_types_core::{components::InstanceKey, ComponentName};
use re_viewer_context::{
    blueprint_timepoint_for_writes, DataResult, SystemCommand, SystemCommandSender as _,
    UiVerbosity, ViewerContext,
};
use re_viewport::SpaceViewBlueprint;

use crate::selection_panel::guess_query_and_store_for_selected_entity;

pub fn override_ui(
    ctx: &ViewerContext<'_>,
    space_view: &SpaceViewBlueprint,
    instance_path: &InstancePath,
    ui: &mut egui::Ui,
) {
    let InstancePath {
        entity_path,
        instance_key,
    } = instance_path;

    let (query, store) = guess_query_and_store_for_selected_entity(ctx, entity_path);

    let query_result = ctx.lookup_query_result(space_view.query_id());
    let Some(data_result) = query_result
        .tree
        .lookup_result_by_path_and_group(&instance_path.entity_path, false)
        .cloned()
    else {
        ui.label(ctx.re_ui.error_text("Entity not found in view."));
        return;
    };

    let active_components: BTreeSet<ComponentName> = data_result
        .property_overrides
        .as_ref()
        .map(|props| props.component_overrides.keys().cloned().collect())
        .unwrap_or_default();

    add_new_override(
        ctx,
        &query,
        store,
        ui,
        space_view,
        &data_result,
        &active_components,
    );

    let Some(overrides) = data_result.property_overrides else {
        return;
    };

    egui::Grid::new("overrides").num_columns(3).show(ui, |ui| {
        for (component_name, (store_kind, entity_path)) in overrides
            .component_overrides
            .iter()
            .sorted_by_key(|(c, _)| *c)
        {
            if !is_component_visible_in_ui(component_name) {
                continue;
            }

            temporary_style_ui_for_component(ui, component_name, |ui| {
                item_ui::component_path_button(
                    ctx,
                    ui,
                    &ComponentPath::new(entity_path.clone(), *component_name),
                );
            });

            let component_data = match store_kind {
                StoreKind::Blueprint => {
                    let store = ctx.store_context.blueprint.store();
                    let query = ctx.blueprint_query;
                    get_component_with_instances(store, query, entity_path, *component_name)
                }
                StoreKind::Recording => {
                    get_component_with_instances(store, &query, entity_path, *component_name)
                }
            };

            if let Some((_, _, component_data)) = component_data {
                ctx.component_ui_registry.edit(
                    ctx,
                    ui,
                    UiVerbosity::Small,
                    &query,
                    store,
                    entity_path,
                    &overrides.override_path,
                    &component_data,
                    instance_key,
                );
            } else {
                // TODO(jleibs): This shouldn't happen. Warn instead?
                ui.weak("(empty)");
            }

            if ui.button("❎").clicked() {
                // Note: need to use the blueprint store since the data might
                // not exist in the recording store.
                ctx.save_empty_blueprint_component_name(
                    ctx.store_context.blueprint.store(),
                    &overrides.override_path,
                    *component_name,
                );
            }

            ui.end_row();
        }
        Some(())
    });
}

pub fn add_new_override(
    ctx: &ViewerContext<'_>,
    query: &LatestAtQuery,
    store: &DataStore,
    ui: &mut egui::Ui,
    space_view: &SpaceViewBlueprint,
    data_result: &DataResult,
    active_components: &BTreeSet<ComponentName>,
) {
    ui.menu_button("Add new override", |ui| {
        ui.style_mut().wrap = Some(false);

        let view_systems = ctx
            .space_view_class_registry
            .new_visualizer_collection(*space_view.class_identifier());

        // We have to have at least 1 visualizer system or we can't create an override
        let Some(system_for_initial_value) = view_systems.systems.values().next() else {
            return;
        };

        let mut components_per_visualizer = data_result
            .visualizers
            .iter()
            .filter_map(|vis| view_systems.get_by_identifier(*vis).ok())
            .map(|vis| vis.visualizer_query_info().queried);

        // Accmulate all the components if there are multiple visualizers
        let mut components = components_per_visualizer.next().unwrap_or_default();
        for mut rest in components_per_visualizer {
            components.append(&mut rest);
        }

        // Empty space views of every available types
        for component in components.difference(active_components) {
            // If we don't have an override_path we can't set up an initial override
            // this shouldn't happen if the `DataResult` is valid.
            let Some(override_path) = data_result.override_path() else {
                continue;
            };

            // If there is no registered editor, don't let the user create an override
            if !ctx.component_ui_registry.has_registered_editor(component) {
                continue;
            }

            if ui.button(component.as_str()).clicked() {
                let components = [*component];

                // TODO(jleibs): The override-editor interface needs a way to specify the default-value
                // if there isn't one in the store already. We can't default to "empty" because empty
                // needs to be "no override."
                let mut splat_cell: DataCell = [InstanceKey::SPLAT].into();
                splat_cell.compute_size_bytes();

                let Some(mut initial_data) = store
                    .latest_at(query, &data_result.entity_path, *component, &components)
                    .and_then(|result| result.2[0].clone())
                    .and_then(|cell| {
                        if cell.num_instances() == 1 {
                            Some(cell)
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        system_for_initial_value.initial_override_value(
                            ctx,
                            query,
                            store,
                            &data_result.entity_path,
                            component,
                        )
                    })
                    .or_else(|| {
                        ctx.component_ui_registry.default_value(
                            ctx,
                            query,
                            store,
                            &data_result.entity_path,
                            component,
                        )
                    })
                else {
                    re_log::warn!("Could not identify an initial value for: {}", component);
                    return;
                };

                initial_data.compute_size_bytes();

                match DataRow::from_cells(
                    RowId::new(),
                    blueprint_timepoint_for_writes(),
                    override_path.clone(),
                    1,
                    [splat_cell, initial_data],
                ) {
                    Ok(row) => ctx
                        .command_sender
                        .send_system(SystemCommand::UpdateBlueprint(
                            ctx.store_context.blueprint.store_id().clone(),
                            vec![row],
                        )),
                    Err(err) => {
                        re_log::warn!("Failed to create DataRow for blueprint component: {}", err);
                    }
                }

                ui.close_menu();
            }
        }
    });
}
