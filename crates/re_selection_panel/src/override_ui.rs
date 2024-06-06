use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;

use re_data_store::LatestAtQuery;
use re_entity_db::{EntityDb, InstancePath};
use re_log_types::{DataCell, DataRow, RowId, StoreKind};
use re_types_core::{components::VisualizerOverrides, ComponentName};
use re_ui::{ContextExt as _, UiExt as _};
use re_viewer_context::{
    ComponentUiTypes, DataResult, OverridePath, QueryContext, SpaceViewClassExt as _,
    SystemCommand, SystemCommandSender as _, ViewContext, ViewSystemIdentifier, ViewerContext,
};
use re_viewport_blueprint::SpaceViewBlueprint;

pub fn override_ui(
    ctx: &ViewContext<'_>,
    space_view: &SpaceViewBlueprint,
    instance_path: &InstancePath,
    ui: &mut egui::Ui,
) {
    let InstancePath {
        entity_path,
        instance: _, // Override ui only works on the first instance of an entity.
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
        ui.label(ui.ctx().error_text("Entity not found in view."));
        return;
    };

    let active_overrides: BTreeSet<ComponentName> = data_result
        .property_overrides
        .as_ref()
        .map(|props| props.resolved_component_overrides.keys().copied().collect())
        .unwrap_or_default();

    let mut component_to_vis: BTreeMap<ComponentName, ViewSystemIdentifier> = Default::default();

    // Accumulate the components across all visualizers and track which visualizer
    // each component came from so we can use it for fallbacks later.
    //
    // If two visualizers have the same component, the first one wins.
    // TODO(jleibs): We can do something fancier in the future such as presenting both
    // options once we have a motivating use-case.
    for vis in &data_result.visualizers {
        let Some(queried) = ctx
            .visualizer_collection
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
        &component_to_vis,
        &active_overrides,
        &data_result,
    );

    let Some(overrides) = data_result.property_overrides else {
        return;
    };

    let sorted_overrides = overrides
        .resolved_component_overrides
        .into_iter()
        .sorted_by_key(|(c, _)| *c);

    re_ui::list_item::list_item_scope(ui, "overrides", |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (
            ref component_name,
            OverridePath {
                ref store_kind,
                path: ref entity_path_overridden,
            },
        ) in sorted_overrides
        {
            let Some(visualizer_identifier) = component_to_vis.get(component_name) else {
                continue;
            };
            let Ok(visualizer) = ctx
                .visualizer_collection
                .get_by_identifier(*visualizer_identifier)
            else {
                re_log::warn!(
                    "Failed to resolve visualizer identifier {visualizer_identifier}, to a visualizer implementation"
                );
                continue;
            };

            let value_fn = |ui: &mut egui::Ui| {
                let (origin_db, query) = match store_kind {
                    StoreKind::Blueprint => {
                        (ctx.blueprint_db(), ctx.viewer_ctx.blueprint_query.clone())
                    }
                    StoreKind::Recording => (ctx.recording(), ctx.current_query()),
                };
                let component_data = origin_db
                    .query_caches()
                    .latest_at(
                        origin_db.store(),
                        &query,
                        entity_path_overridden,
                        [*component_name],
                    )
                    .components
                    .get(component_name)
                    .cloned(); /* arc */

                if let Some(results) = component_data {
                    ctx.viewer_ctx.component_ui_registry.singleline_edit_ui(
                        &QueryContext {
                            view_ctx: ctx,
                            target_entity_path: &instance_path.entity_path,
                            archetype_name: None,
                            query: &query,
                        },
                        ui,
                        origin_db,
                        entity_path_overridden,
                        *component_name,
                        &results,
                        visualizer.as_fallback_provider(),
                    );
                } else {
                    // TODO(jleibs): Is it possible to set an override to empty and not confuse
                    // the situation with "not-overridden?". Maybe we hit this in cases of `[]` vs `[null]`.
                    ui.weak("(empty)");
                }
            };

            ui.list_item()
                .interactive(false)
                .show_flat(
                    ui,
                    re_ui::list_item::PropertyContent::new(component_name.short_name())
                        .min_desired_width(150.0)
                        .action_button(&re_ui::icons::CLOSE, || {
                            ctx.save_empty_blueprint_component_by_name(
                                &overrides.individual_override_path,
                                *component_name,
                            );
                        })
                        .value_fn(|ui, _| value_fn(ui)),
                )
                .on_hover_text(component_name.full_name());
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn add_new_override(
    ctx: &ViewContext<'_>,
    query: &LatestAtQuery,
    db: &EntityDb,
    ui: &mut egui::Ui,
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
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                let query_context = QueryContext {
                    view_ctx: ctx,
                    target_entity_path: &data_result.entity_path,
                    archetype_name: None,
                    query,
                };

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
                    // TODO(andreas): Can only handle single line editors right now.
                    if !ctx
                        .viewer_ctx
                        .component_ui_registry
                        .registered_ui_types(*component)
                        .contains(ComponentUiTypes::SingleLineEditor)
                    {
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
                                ctx.visualizer_collection
                                    .get_by_identifier(*viz)
                                    .ok()
                                    .and_then(|sys| {
                                        sys.fallback_for(&query_context, *component)
                                            .map(|fallback| {
                                                DataCell::from_arrow(*component, fallback)
                                            })
                                            .ok()
                                    })
                            })
                        else {
                            re_log::warn!("Could not identify an initial value for: {}", component);
                            return;
                        };

                        initial_data.compute_size_bytes();

                        match DataRow::from_cells(
                            RowId::new(),
                            ctx.blueprint_timepoint_for_writes(),
                            override_path.clone(),
                            [initial_data],
                        ) {
                            Ok(row) => {
                                ctx.viewer_ctx.command_sender.send_system(
                                    SystemCommand::UpdateBlueprint(
                                        ctx.blueprint_db().store_id().clone(),
                                        vec![row],
                                    ),
                                );
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
            ui.label(ui.ctx().error_text("Entity not found in view."));
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

        re_ui::list_item::list_item_scope(ui, "visualizers", |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            for viz_name in &active_visualizers {
                ui.list_item().interactive(false).show_flat(
                    ui,
                    re_ui::list_item::LabelContent::new(viz_name.as_str())
                        .min_desired_width(150.0)
                        .with_buttons(|ui| {
                            let response = ui.small_icon_button(&re_ui::icons::CLOSE);
                            if response.clicked() {
                                let component = VisualizerOverrides::from(
                                    active_visualizers
                                        .iter()
                                        .filter(|v| *v != viz_name)
                                        .map(|v| re_types_core::ArrowString::from(v.as_str()))
                                        .collect::<Vec<_>>(),
                                );

                                ctx.save_blueprint_component(override_path, &component);
                            }
                            response
                        })
                        .always_show_buttons(true),
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

    let visualizable_entities = space_view
        .class(ctx.space_view_class_registry)
        .determine_visualizable_entities(
            &applicable_entities_per_visualizer,
            entity_db,
            &ctx.space_view_class_registry
                .new_visualizer_collection(space_view.class_identifier()),
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
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
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
