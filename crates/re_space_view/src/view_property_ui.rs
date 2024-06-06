use ahash::HashMap;
use re_types_core::{Archetype, ArchetypeFieldInfo, ArchetypeInfo, ComponentName};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::{
    ComponentFallbackProvider, ComponentUiTypes, QueryContext, SpaceViewId, ViewContext,
    ViewerContext,
};
use re_viewport_blueprint::entity_path_for_view_property;

/// Display the UI for editing all components of a blueprint archetype.
///
/// Note that this will show default values for components that are null.
pub fn view_property_ui<A: Archetype>(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    view_id: SpaceViewId,
    fallback_provider: &dyn ComponentFallbackProvider,
) {
    view_property_ui_impl(ctx, ui, view_id, A::info(), fallback_provider);
}

fn view_property_ui_impl(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
    view_id: SpaceViewId,
    archetype: ArchetypeInfo,
    fallback_provider: &dyn ComponentFallbackProvider,
) {
    let blueprint_path =
        entity_path_for_view_property(view_id, ctx.blueprint_db().tree(), archetype.name);
    let query_ctx = QueryContext {
        view_ctx: ctx,
        target_entity_path: &blueprint_path,
        archetype_name: Some(archetype.name),
        query: ctx.viewer_ctx.blueprint_query,
    };

    let component_results = ctx.blueprint_db().latest_at(
        ctx.viewer_ctx.blueprint_query,
        &blueprint_path,
        archetype.component_names.iter().copied(),
    );

    let field_info_per_component: HashMap<_, _> = archetype
        .field_infos
        .map(|field_infos| {
            field_infos
                .iter()
                .cloned()
                .map(|field_info| (field_info.component_name, field_info))
                .collect()
        })
        .unwrap_or_default();

    let non_indicator_components = archetype
        .component_names
        .as_ref()
        .iter()
        .filter(|component_name| !component_name.is_indicator_component())
        .collect::<Vec<_>>();

    // If the property archetype only has a single component, don't show an additional hierarchy level!
    if non_indicator_components.len() == 1 {
        let component_name = *non_indicator_components[0];
        let field_info = field_info_per_component.get(&component_name);

        view_property_component_ui(
            &query_ctx,
            ui,
            component_name,
            archetype.display_name,
            field_info,
            &blueprint_path,
            component_results.get_or_empty(component_name),
            fallback_provider,
        );
    } else {
        let sub_prop_ui = |ui: &mut egui::Ui| {
            for component_name in non_indicator_components {
                let field_info = field_info_per_component.get(component_name);
                let display_name = field_info
                    .map_or_else(|| component_name.short_name(), |info| info.display_name);

                view_property_component_ui(
                    &query_ctx,
                    ui,
                    *component_name,
                    display_name,
                    field_info,
                    &blueprint_path,
                    component_results.get_or_empty(*component_name),
                    fallback_provider,
                );
            }
        };

        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(archetype.name.full_name()),
                true,
                list_item::LabelContent::new(archetype.display_name),
                sub_prop_ui,
            );
    }
}

/// Draw view property ui for a single component of a view property archetype.
#[allow(clippy::too_many_arguments)]
fn view_property_component_ui(
    ctx: &QueryContext<'_>,
    ui: &mut egui::Ui,
    component_name: ComponentName,
    root_item_display_name: &str,
    field_info: Option<&ArchetypeFieldInfo>,
    blueprint_path: &re_log_types::EntityPath,
    component_results: &re_query::LatestAtComponentResults,
    fallback_provider: &dyn ComponentFallbackProvider,
) {
    let singleline_list_item_content = singleline_list_item_content(
        ctx,
        root_item_display_name,
        blueprint_path,
        component_name,
        component_results,
        fallback_provider,
    );

    let ui_types = ctx
        .view_ctx
        .viewer_ctx
        .component_ui_registry
        .registered_ui_types(component_name);

    let mut list_item_response = if ui_types.contains(ComponentUiTypes::MultiLineEditor) {
        let default_open = false;
        let id = egui::Id::new((blueprint_path.hash(), component_name));
        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                id,
                default_open,
                singleline_list_item_content,
                |ui| {
                    ctx.view_ctx
                        .viewer_ctx
                        .component_ui_registry
                        .multiline_edit_ui(
                            ctx,
                            ui,
                            ctx.view_ctx.blueprint_db(),
                            blueprint_path,
                            component_name,
                            component_results,
                            fallback_provider,
                        );
                },
            )
            .item_response
    } else {
        ui.list_item()
            .interactive(false)
            // It might have siblings that have a hierarchy.
            .show_hierarchical(ui, singleline_list_item_content)
    };

    if let Some(tooltip) = field_info.map(|info| info.documentation) {
        list_item_response = list_item_response.on_hover_text(tooltip);
    }

    view_property_context_menu(
        ctx.view_ctx.viewer_ctx,
        &list_item_response,
        blueprint_path,
        component_name,
        component_results,
    );
}

fn view_property_context_menu(
    ctx: &ViewerContext<'_>,
    list_item_response: &egui::Response,
    blueprint_path: &re_log_types::EntityPath,
    component_name: ComponentName,
    component_results: &re_query::LatestAtComponentResults,
) {
    list_item_response.context_menu(|ui| {
        if ui.button("Reset to default blueprint.")
        .on_hover_text("Resets this property to the value in the default blueprint.\n
        If no default blueprint was set or it didn't set any value for this field, this is the same as resetting to empty.")
        .clicked() {
            ctx.reset_blueprint_component_by_name(blueprint_path, component_name);
            ui.close_menu();
        }

        let blueprint_db = ctx.blueprint_db();
        ui.add_enabled_ui(!component_results.is_empty(blueprint_db.resolver()), |ui| {
            if ui.button("Reset to empty.")
                .on_hover_text("Resets this property to an unset value, meaning that a heuristically determined value will be used instead.\n
This has the same effect as not setting the value in the blueprint at all.")
                .on_disabled_hover_text("The property is already unset.")
                .clicked() {
                ctx.save_empty_blueprint_component_by_name(blueprint_path, component_name);
                ui.close_menu();
            }
        });

        // TODO(andreas): The next logical thing here is now to save it to the default blueprint!
        // This should be fairly straight forward except that we need to make sure that a default blueprint exists in the first place.
    });
}

fn singleline_list_item_content<'a>(
    ctx: &'a QueryContext<'_>,
    display_name: &str,
    blueprint_path: &'a re_log_types::EntityPath,
    component_name: ComponentName,
    component_results: &'a re_query::LatestAtComponentResults,
    fallback_provider: &'a dyn ComponentFallbackProvider,
) -> list_item::PropertyContent<'a> {
    list_item::PropertyContent::new(display_name)
        .action_button(&re_ui::icons::RESET, move || {
            ctx.view_ctx
                .viewer_ctx
                .reset_blueprint_component_by_name(blueprint_path, component_name);
        })
        .value_fn(move |ui, _| {
            ctx.view_ctx
                .viewer_ctx
                .component_ui_registry
                .singleline_edit_ui(
                    ctx,
                    ui,
                    ctx.view_ctx.blueprint_db(),
                    blueprint_path,
                    component_name,
                    component_results,
                    fallback_provider,
                );
        })
}
