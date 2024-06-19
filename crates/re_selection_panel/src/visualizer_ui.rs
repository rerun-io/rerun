use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;

use re_data_store::LatestAtQuery;
use re_data_ui::DataUi;
use re_entity_db::{EntityDb, InstancePath};
use re_log_types::{DataCell, DataRow, EntityPath, RowId};
use re_space_view::latest_at_with_blueprint_resolved_data;
use re_types_core::{components::VisualizerOverrides, ComponentName};
use re_ui::{list_item, ContextExt as _, UiExt as _};
use re_viewer_context::{
    ComponentUiTypes, DataResult, OverridePath, SpaceViewClassExt as _, SystemCommand,
    SystemCommandSender as _, UiLayout, ViewContext, ViewSystemIdentifier,
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

    // TODO(jleibs): This clone is annoying, but required because QueryContext wants
    // a reference to the EntityPath. We could probably refactor this to avoid the clone.
    let Some(overrides) = data_result.property_overrides.clone() else {
        return;
    };

    let sorted_overrides = overrides
        .resolved_component_overrides
        .into_iter()
        .sorted_by_key(|(c, _)| *c);

    let query_context = ctx.query_context(&data_result, &query);

    list_item::list_item_scope(ui, "overrides", |ui| {
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

                let query_context = ctx.query_context(data_result, query);

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

pub fn visualizer_ui(
    ctx: &ViewContext<'_>,
    space_view: &SpaceViewBlueprint,
    entity_path: &EntityPath,
    ui: &mut egui::Ui,
) {
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

    let remove_visualizer_button = |ui: &mut egui::Ui, vis_name: ViewSystemIdentifier| {
        let response = ui.small_icon_button(&re_ui::icons::CLOSE);
        if response.clicked() {
            let component = VisualizerOverrides::from(
                active_visualizers
                    .iter()
                    .filter(|v| *v != &vis_name)
                    .map(|v| re_types_core::ArrowString::from(v.as_str()))
                    .collect::<Vec<_>>(),
            );

            ctx.save_blueprint_component(override_path, &component);
        }
        response
    };

    list_item::list_item_scope(ui, "visualizers", |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;

        for &visualizer_id in &active_visualizers {
            let default_open = true;
            ui.list_item()
                .interactive(false)
                .show_hierarchical_with_children(
                    ui,
                    ui.make_persistent_id(visualizer_id),
                    default_open,
                    list_item::LabelContent::new(visualizer_id.as_str())
                        .min_desired_width(150.0)
                        .with_buttons(|ui| remove_visualizer_button(ui, visualizer_id))
                        .always_show_buttons(true),
                    |ui| visualizer_components(ctx, ui, &data_result, visualizer_id),
                );
        }
    });
}

enum ValueSource {
    Override,
    Store,
    //AnnotationContext, // TODO: Here be dragons
    Default,
    FallbackOrPlaceholder,
}

fn visualizer_components(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    data_result: &DataResult,
    visualizer_id: ViewSystemIdentifier,
) {
    // List all components that the visualizer may consume.
    let Ok(visualizer) = ctx.visualizer_collection.get_by_identifier(visualizer_id) else {
        re_log::warn!(
            "Failed to resolve visualizer identifier {visualizer_id}, to a visualizer implementation"
        );
        return;
    };

    let query_info = visualizer.visualizer_query_info();

    let store_query = ctx.current_query();
    let query_ctx = ctx.query_context(data_result, &store_query);

    // Query fully resolved data.
    let query_result = latest_at_with_blueprint_resolved_data(
        ctx,
        None, // TODO(andreas): Figure out how to deal with annotation context here.
        &store_query,
        data_result,
        query_info.queried.iter().copied(),
    );

    // TODO(andreas): Should we show required components in a special way?
    for &component in &query_info.queried {
        if component.is_indicator_component() {
            continue;
        }

        // TODO(andreas): What about annotation context?

        // Query all the sources for our value.
        // (technically we only need to query those that are shown, but rolling this out makes things easier).
        let result_override = query_result.overrides.get(component);
        let raw_override = result_override.and_then(|r| r.try_raw(&query_result.resolver));
        let non_empty_override = raw_override.as_ref().map_or(false, |r| !r.is_empty());

        let result_store = query_result.results.get(component);
        let raw_store = result_store.and_then(|r| r.try_raw(&query_result.resolver));
        let non_empty_store = raw_store.as_ref().map_or(false, |r| !r.is_empty());

        let result_default = query_result.defaults.get(component);
        let raw_default = result_default.and_then(|r| r.try_raw(&query_result.resolver));
        let non_empty_default = raw_default.as_ref().map_or(false, |r| !r.is_empty());

        let raw_fallback = match visualizer.fallback_for(&query_ctx, component) {
            Ok(fallback) => fallback,
            Err(err) => {
                re_log::warn_once!("Failed to get fallback for component {component}: {err}");
                continue; // TODO(andreas): Don't give up on the entire component because of this.
            }
        };

        // Determine where the final value comes from.
        // Putting this into an enum makes it easier to reason about the next steps.
        let value_source = match (non_empty_override, non_empty_store, non_empty_default) {
            (true, _, _) => ValueSource::Override,
            (false, true, _) => ValueSource::Store,
            (false, false, true) => ValueSource::Default,
            (false, false, false) => ValueSource::FallbackOrPlaceholder,
        };

        #[allow(clippy::unwrap_used)] // We checked earlier that these values are valid!
        let raw_current_value = match value_source {
            ValueSource::Override => raw_override.unwrap(),
            ValueSource::Store => raw_store.unwrap(),
            ValueSource::Default => raw_default.unwrap(),
            ValueSource::FallbackOrPlaceholder => raw_fallback,
        };

        let Some(override_path) = data_result.individual_override_path() else {
            // This shouldn't the `DataResult` is valid.
            if cfg!(debug_assertions) {
                re_log::error!("No override path for entity: {}", data_result.entity_path);
            }
            return;
        };

        let value_fn = |ui: &mut egui::Ui, _style| {
            // Edit ui can only handle a single value.
            let multiline = false;
            if raw_current_value.len() > 1
                || !ctx.viewer_ctx.component_ui_registry.try_show_edit_ui(
                    ctx.viewer_ctx,
                    ui,
                    raw_current_value.as_ref(),
                    override_path,
                    component,
                    multiline,
                )
            {
                // TODO(andreas): Unfortunately, display ui wants to do the query itself.
                // In fact some display UIs will struggle since they try to query additional data from the store.
                // We pass
                // so we have to figure out what store and path things come from.
                let bp_query = ctx.viewer_ctx.blueprint_query;

                #[allow(clippy::unwrap_used)] // We checked earlier that these values are valid!
                let (query, db, entity_path, latest_at_results) = match value_source {
                    ValueSource::Override => (
                        bp_query,
                        ctx.blueprint_db(),
                        override_path,
                        result_override.unwrap(),
                    ),
                    ValueSource::Store => (
                        &store_query,
                        ctx.recording(),
                        &data_result.entity_path,
                        result_store.unwrap(),
                    ),
                    ValueSource::Default => (
                        bp_query,
                        ctx.blueprint_db(),
                        ctx.defaults_path,
                        result_default.unwrap(),
                    ),
                    ValueSource::FallbackOrPlaceholder => {
                        // TODO(andreas): this isn't in any store, can't display this.
                        ui.weak("can't display some fallback values");
                        return;
                    }
                };

                re_data_ui::EntityLatestAtResults {
                    entity_path: entity_path.clone(),
                    results: latest_at_results,
                }
                .data_ui(ctx.viewer_ctx, ui, UiLayout::List, query, db);
            }
        };

        // TODO: action button.

        let content = list_item::PropertyContent::new(component.short_name()).value_fn(value_fn);
        // TODO: edit ui and actual value.

        ui.list_item()
            .interactive(false)
            .show_flat(ui, content)
            .on_hover_text(component.full_name());
    }
}

fn add_new_visualizer(
    ctx: &ViewContext<'_>,
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
        .viewer_ctx
        .space_view_class_registry
        .applicable_entities_for_visualizer_systems(entity_db.store_id());

    let visualizable_entities = space_view
        .class(ctx.viewer_ctx.space_view_class_registry)
        .determine_visualizable_entities(
            &applicable_entities_per_visualizer,
            entity_db,
            &ctx.visualizer_collection,
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
