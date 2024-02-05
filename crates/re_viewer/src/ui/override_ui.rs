use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use re_data_store::{DataStore, LatestAtQuery};
use re_data_ui::{is_component_visible_in_ui, item_ui, temporary_style_ui_for_component};
use re_entity_db::{EntityDb, InstancePath};
use re_log_types::{ComponentPath, DataCell, DataRow, RowId, StoreKind};
use re_query_cache::external::re_query::get_component_with_instances;
use re_space_view::SpaceViewBlueprint;
use re_types_core::components::VisualizerOverrides;
use re_types_core::{components::InstanceKey, ComponentName};
use re_viewer_context::{
    blueprint_timepoint_for_writes, DataResult, SystemCommand, SystemCommandSender as _,
    UiVerbosity, ViewSystemIdentifier, ViewerContext,
};
use re_viewport::determine_visualizable_entities;

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

    // Because of how overrides are implemented the overridden-data must be an entity
    // in the real store. We would never show an override UI for a selected blueprint
    // entity from the blueprint-inspector since it isn't "part" of a space-view to provide
    // the overrides.
    let query = ctx.current_query();
    let store = ctx.entity_db.store();

    let query_result = ctx.lookup_query_result(space_view.query_id());
    let Some(data_result) = query_result
        .tree
        .lookup_result_by_path_and_group(entity_path, false)
        .cloned()
    else {
        ui.label(ctx.re_ui.error_text("Entity not found in view."));
        return;
    };

    let active_overrides: BTreeSet<ComponentName> = data_result
        .property_overrides
        .as_ref()
        .map(|props| props.component_overrides.keys().cloned().collect())
        .unwrap_or_default();

    let view_systems = ctx
        .space_view_class_registry
        .new_visualizer_collection(*space_view.class_identifier());

    let mut component_to_vis: BTreeMap<ComponentName, ViewSystemIdentifier> = Default::default();

    // Accumulate the components across all visualizers and track which visualizer
    // each component came from so we can use it for defaults later.
    //
    // If two visualizers have the same component, the first one wins.
    // TODO(jleibs): We can do something fancier in the future such as presenting both
    // options once we have a motivating use-case.
    for vis in &data_result.visualizers {
        let Some(queried) = view_systems
            .get_by_identifier(*vis)
            .ok()
            .map(|vis| vis.visualizer_query_info().queried)
        else {
            continue;
        };

        for component in queried {
            component_to_vis.entry(component).or_insert_with(|| *vis);
        }
    }

    add_new_override(
        ctx,
        &query,
        store,
        ui,
        &view_systems,
        &component_to_vis,
        &active_overrides,
        &data_result,
    );

    ui.end_row();

    let Some(overrides) = data_result.property_overrides else {
        return;
    };

    let components: Vec<_> = overrides
        .component_overrides
        .into_iter()
        .sorted_by_key(|(c, _)| *c)
        .filter(|(c, _)| component_to_vis.contains_key(c) && is_component_visible_in_ui(c))
        .collect();

    egui_extras::TableBuilder::new(ui)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .auto_shrink([false, true])
        .column(egui_extras::Column::auto())
        .column(egui_extras::Column::auto())
        .column(egui_extras::Column::remainder())
        .body(|mut body| {
            re_ui::ReUi::setup_table_body(&mut body);
            let row_height = re_ui::ReUi::table_line_height();
            body.rows(row_height, components.len(), |mut row| {
                if let Some((component_name, (store_kind, entity_path))) =
                    components.get(row.index())
                {
                    // Remove button
                    row.col(|ui| {
                        if ctx
                            .re_ui
                            .small_icon_button(ui, &re_ui::icons::CLOSE)
                            .clicked()
                        {
                            // Note: need to use the blueprint store since the data might
                            // not exist in the recording store.
                            ctx.save_empty_blueprint_component_name(
                                ctx.store_context.blueprint.store(),
                                &overrides.override_path,
                                *component_name,
                            );
                        }
                    });
                    // Component label
                    row.col(|ui| {
                        temporary_style_ui_for_component(ui, component_name, |ui| {
                            item_ui::component_path_button(
                                ctx,
                                ui,
                                &ComponentPath::new(entity_path.clone(), *component_name),
                            );
                        });
                    });
                    // Editor last to take up remainder of space
                    row.col(|ui| {
                        let component_data = match store_kind {
                            StoreKind::Blueprint => {
                                let store = ctx.store_context.blueprint.store();
                                let query = ctx.blueprint_query;
                                get_component_with_instances(
                                    store,
                                    query,
                                    entity_path,
                                    *component_name,
                                )
                            }
                            StoreKind::Recording => get_component_with_instances(
                                store,
                                &query,
                                entity_path,
                                *component_name,
                            ),
                        };

                        if let Some((_, _, component_data)) = component_data {
                            ctx.component_ui_registry.edit_ui(
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
                            // TODO(jleibs): Is it possible to set an override to empty and not confuse
                            // the situation with "not-overridden?". Maybe we hit this in cases of `[]` vs `[null]`.
                            ui.weak("(empty)");
                        }
                    });
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
pub fn add_new_override(
    ctx: &ViewerContext<'_>,
    query: &LatestAtQuery,
    store: &DataStore,
    ui: &mut egui::Ui,
    view_systems: &re_viewer_context::VisualizerCollection,
    component_to_vis: &BTreeMap<ComponentName, ViewSystemIdentifier>,
    active_overrides: &BTreeSet<ComponentName>,
    data_result: &DataResult,
) {
    let remaining_components = component_to_vis
        .keys()
        .filter(|c| !active_overrides.contains(*c))
        .collect::<Vec<_>>();

    let enabled = !remaining_components.is_empty();

    ui.add_enabled_ui(enabled, |ui| {
        let mut opened = false;
        let menu = ui
            .menu_button("Add", |ui| {
                opened = true;
                ui.style_mut().wrap = Some(false);

                // Present the option to add new components for each component that doesn't
                // already have an active override.
                for (component, viz) in component_to_vis {
                    if active_overrides.contains(component) {
                        continue;
                    }
                    // If we don't have an override_path we can't set up an initial override
                    // this shouldn't happen if the `DataResult` is valid.
                    let Some(override_path) = data_result.override_path() else {
                        if cfg!(debug_assertions) {
                            re_log::error!("No override path for: {}", component);
                        }
                        continue;
                    };

                    // If there is no registered editor, don't let the user create an override
                    if !ctx.component_ui_registry.has_registered_editor(component) {
                        continue;
                    }

                    if ui.button(component.short_name()).clicked() {
                        // We are creating a new override. We need to decide what initial value to give it.
                        // - First see if there's an existing splat in the recording.
                        // - Next see if visualizer system wants to provide a value.
                        // - Finally, fall back on the default value from the component registry.

                        let components = [*component];

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
                                view_systems.get_by_identifier(*viz).ok().and_then(|sys| {
                                    sys.initial_override_value(
                                        ctx,
                                        query,
                                        store,
                                        &data_result.entity_path,
                                        component,
                                    )
                                })
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
                            Ok(row) => {
                                ctx.command_sender
                                    .send_system(SystemCommand::UpdateBlueprint(
                                        ctx.store_context.blueprint.store_id().clone(),
                                        vec![row],
                                    ));
                            }
                            Err(err) => {
                                re_log::warn!(
                                    "Failed to create DataRow for blueprint component: {}",
                                    err
                                );
                            }
                        }

                        ui.close_menu();
                    }
                }
            })
            .response
            .on_disabled_hover_text("No additional components available.");
        if !opened {
            menu.on_hover_text("Choose a component to specify an override value.".to_owned());
        }
    });
}

// ---

pub fn override_visualizer_ui(
    ctx: &ViewerContext<'_>,
    space_view: &SpaceViewBlueprint,
    instance_path: &InstancePath,
    ui: &mut egui::Ui,
) {
    ui.push_id("visualizer_overrides", |ui| {
        let InstancePath {
            entity_path,
            instance_key: _,
        } = instance_path;

        let entity_db = ctx.entity_db;

        let query_result = ctx.lookup_query_result(space_view.query_id());
        let Some(data_result) = query_result
            .tree
            .lookup_result_by_path_and_group(entity_path, false)
            .cloned()
        else {
            ui.label(ctx.re_ui.error_text("Entity not found in view."));
            return;
        };

        let Some(override_path) = data_result.override_path() else {
            if cfg!(debug_assertions) {
                re_log::error!("No override path for entity: {}", data_result.entity_path);
            }
            return;
        };

        let active_visualizers: Vec<_> = data_result.visualizers.iter().sorted().cloned().collect();

        add_new_visualizer(
            ctx,
            entity_db,
            ui,
            space_view,
            &data_result,
            &active_visualizers,
        );

        ui.end_row();

        egui_extras::TableBuilder::new(ui)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .auto_shrink([false, true])
            .column(egui_extras::Column::auto())
            .column(egui_extras::Column::remainder())
            .body(|mut body| {
                re_ui::ReUi::setup_table_body(&mut body);
                let row_height = re_ui::ReUi::table_line_height();
                body.rows(row_height, active_visualizers.len(), |mut row| {
                    if let Some(vis_name) = active_visualizers.get(row.index()) {
                        // Remove button
                        row.col(|ui| {
                            if ctx
                                .re_ui
                                .small_icon_button(ui, &re_ui::icons::CLOSE)
                                .clicked()
                            {
                                let component = VisualizerOverrides::from(
                                    active_visualizers
                                        .iter()
                                        .filter(|v| *v != vis_name)
                                        .map(|v| {
                                            let arrow_str: re_types_core::ArrowString =
                                                v.as_str().into();
                                            arrow_str
                                        })
                                        .collect::<Vec<_>>(),
                                );

                                ctx.save_blueprint_component(override_path, component);
                            }
                        });
                        // Visualizer label
                        row.col(|ui| {
                            ui.label(vis_name.as_str());
                        });
                    }
                });
            });
    });
}

pub fn add_new_visualizer(
    ctx: &ViewerContext<'_>,
    entity_db: &EntityDb,
    ui: &mut egui::Ui,
    space_view: &SpaceViewBlueprint,
    data_result: &DataResult,
    active_visualizers: &[ViewSystemIdentifier],
) {
    // If we don't have an override_path we can't set up an initial override
    // this shouldn't happen if the `DataResult` is valid.
    let Some(override_path) = data_result.override_path() else {
        if cfg!(debug_assertions) {
            re_log::error!("No override path for entity: {}", data_result.entity_path);
        }
        return;
    };

    // TODO(jleibs): This has already been computed for the SpaceView this frame. Maybe We
    // should do this earlier and store it with the SpaceView?
    let applicable_entities_per_visualizer = ctx
        .space_view_class_registry
        .applicable_entities_for_visualizer_systems(entity_db.store_id());

    let visualizable_entities = determine_visualizable_entities(
        &applicable_entities_per_visualizer,
        entity_db,
        &ctx.space_view_class_registry
            .new_visualizer_collection(*space_view.class_identifier()),
        space_view.class(ctx.space_view_class_registry),
        &space_view.space_origin,
    );

    let visualizer_options = visualizable_entities
        .iter()
        .filter(|(vis, ents)| {
            ents.contains(&data_result.entity_path) && !active_visualizers.contains(vis)
        })
        .map(|(vis, _)| vis)
        .sorted()
        .collect::<Vec<_>>();

    let enabled = !visualizer_options.is_empty();

    let mut opened = false;

    ui.add_enabled_ui(enabled, |ui| {
        let menu = ui
            .menu_button("Add", |ui| {
                ui.style_mut().wrap = Some(false);
                opened = true;

                if visualizer_options.is_empty() {
                    ui.close_menu();
                }

                // Present the option to add new components for each component that doesn't
                // already have an active override.
                for viz in visualizer_options {
                    if ui.button(viz.as_str()).clicked() {
                        let component = VisualizerOverrides::from(
                            active_visualizers
                                .iter()
                                .chain(std::iter::once(viz))
                                .map(|v| {
                                    let arrow_str: re_types_core::ArrowString = v.as_str().into();
                                    arrow_str
                                })
                                .collect::<Vec<_>>(),
                        );

                        ctx.save_blueprint_component(override_path, component);

                        ui.close_menu();
                    }
                }
            })
            .response
            .on_disabled_hover_text("No additional visualizers available.");

        if !opened {
            menu.on_hover_text("Choose a component to specify an override value.".to_owned());
        }
    });
}
