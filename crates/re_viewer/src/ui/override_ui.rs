use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;

use re_data_store::LatestAtQuery;
use re_entity_db::{EntityDb, InstancePath};
use re_log_types::{DataRow, RowId, StoreKind};
use re_space_view::{determine_visualizable_entities, SpaceViewBlueprint};
use re_types_core::{components::VisualizerOverrides, ComponentName};
use re_viewer_context::{
    DataResult, OverridePath, SystemCommand, SystemCommandSender as _, UiVerbosity,
    ViewSystemIdentifier, ViewerContext,
};

pub fn override_ui(
    ctx: &ViewerContext<'_>,
    space_view: &SpaceViewBlueprint,
    instance_path: &InstancePath,
    ui: &mut egui::Ui,
) {
    let InstancePath {
        entity_path,
        instance,
    } = instance_path;

    // Because of how overrides are implemented the overridden-data must be an entity
    // in the real store. We would never show an override UI for a selected blueprint
    // entity from the blueprint-inspector since it isn't "part" of a space-view to provide
    // the overrides.
    let query = ctx.current_query();

    let query_result = ctx.lookup_query_result(space_view.id);
    let Some(data_result) = query_result
        .tree
        .lookup_result_by_path(entity_path)
        .cloned()
    else {
        ui.label(ctx.re_ui.error_text("Entity not found in view."));
        return;
    };

    let active_overrides: BTreeSet<ComponentName> = data_result
        .property_overrides
        .as_ref()
        .map(|props| props.resolved_component_overrides.keys().copied().collect())
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
        ctx.recording(),
        ui,
        &view_systems,
        &component_to_vis,
        &active_overrides,
        &data_result,
    );

    let Some(overrides) = data_result.property_overrides else {
        return;
    };

    let components = overrides
        .resolved_component_overrides
        .into_iter()
        .sorted_by_key(|(c, _)| *c)
        .filter(|(c, _)| component_to_vis.contains_key(c));

    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (
            ref component_name,
            OverridePath {
                ref store_kind,
                path: ref entity_path_overridden,
            },
        ) in components
        {
            let value_fn = |ui: &mut egui::Ui| {
                // TODO(ab): we should use the built in value feature of PropertyContent instead of
                // reinventing the wheel.
                let component_data = match store_kind {
                    StoreKind::Blueprint => {
                        let store = ctx.store_context.blueprint.store();
                        let query = ctx.blueprint_query;
                        ctx.store_context
                            .blueprint
                            .query_caches()
                            .latest_at(store, query, entity_path_overridden, [*component_name])
                            .components
                            .get(component_name)
                            .cloned() /* arc */
                    }
                    StoreKind::Recording => {
                        ctx.recording()
                            .query_caches()
                            .latest_at(
                                ctx.recording_store(),
                                &query,
                                entity_path_overridden,
                                [*component_name],
                            )
                            .components
                            .get(component_name)
                            .cloned() /* arc */
                    }
                };

                if let Some(results) = component_data {
                    ctx.component_ui_registry.edit_ui(
                        ctx,
                        ui,
                        UiVerbosity::Small,
                        &query,
                        ctx.recording(),
                        entity_path_overridden,
                        &overrides.individual_override_path,
                        &results,
                        instance,
                    );
                } else {
                    // TODO(jleibs): Is it possible to set an override to empty and not confuse
                    // the situation with "not-overridden?". Maybe we hit this in cases of `[]` vs `[null]`.
                    ui.weak("(empty)");
                }
            };

            ctx.re_ui
                .list_item2()
                .interactive(false)
                .show_flat(
                    ui,
                    re_ui::list_item2::PropertyContent::new(component_name.short_name())
                        .action_button(&re_ui::icons::CLOSE, || {
                            ctx.save_empty_blueprint_component_name(
                                &overrides.individual_override_path,
                                *component_name,
                            );
                        })
                        .value_fn(|_, ui, _| value_fn(ui)),
                )
                .on_hover_text(component_name.full_name());
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn add_new_override(
    ctx: &ViewerContext<'_>,
    query: &LatestAtQuery,
    db: &EntityDb,
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
                    let Some(override_path) = data_result.individual_override_path() else {
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

                        let Some(mut initial_data) = db
                            .store()
                            .latest_at(query, &data_result.entity_path, *component, &components)
                            .and_then(|result| result.2[0].clone())
                            .or_else(|| {
                                view_systems.get_by_identifier(*viz).ok().and_then(|sys| {
                                    sys.initial_override_value(
                                        ctx,
                                        query,
                                        db.store(),
                                        &data_result.entity_path,
                                        component,
                                    )
                                })
                            })
                            .or_else(|| {
                                ctx.component_ui_registry.default_value(
                                    ctx,
                                    query,
                                    db,
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
                            ctx.store_context.blueprint_timepoint_for_writes(),
                            override_path.clone(),
                            [initial_data],
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
            instance: _,
        } = instance_path;

        let recording = ctx.recording();

        let query_result = ctx.lookup_query_result(space_view.id);
        let Some(data_result) = query_result
            .tree
            .lookup_result_by_path(entity_path)
            .cloned()
        else {
            ui.label(ctx.re_ui.error_text("Entity not found in view."));
            return;
        };

        let Some(override_path) = data_result.individual_override_path() else {
            if cfg!(debug_assertions) {
                re_log::error!("No override path for entity: {}", data_result.entity_path);
            }
            return;
        };

        let active_visualizers: Vec<_> = data_result.visualizers.iter().sorted().copied().collect();

        add_new_visualizer(
            ctx,
            recording,
            ui,
            space_view,
            &data_result,
            &active_visualizers,
        );

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            for viz_name in &active_visualizers {
                ctx.re_ui.list_item2().interactive(false).show_flat(
                    ui,
                    // TODO(ab): use LabelContent instead, but it needs to have an option for the
                    // same action button as PropertyContent.
                    re_ui::list_item2::PropertyContent::new(viz_name.as_str()).action_button(
                        &re_ui::icons::CLOSE,
                        || {
                            let component = VisualizerOverrides::from(
                                active_visualizers
                                    .iter()
                                    .filter(|v| *v != viz_name)
                                    .map(|v| re_types_core::ArrowString::from(v.as_str()))
                                    .collect::<Vec<_>>(),
                            );

                            ctx.save_blueprint_component(override_path, &component);
                        },
                    ),
                );
            }
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
    let Some(override_path) = data_result.individual_override_path() else {
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

                        ctx.save_blueprint_component(override_path, &component);

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
