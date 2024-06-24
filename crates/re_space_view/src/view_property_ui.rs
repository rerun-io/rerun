use re_types_core::{
    reflection::{ArchetypeFieldReflection, ArchetypeReflection},
    Archetype, ArchetypeName, ComponentName,
};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::{
    ComponentFallbackProvider, ComponentUiTypes, QueryContext, SpaceViewId, SpaceViewState,
    ViewerContext,
};
use re_viewport_blueprint::entity_path_for_view_property;

/// Display the UI for editing all components of a blueprint archetype.
///
/// Note that this will show default values for components that are null.
pub fn view_property_ui<A: Archetype>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_id: SpaceViewId,
    fallback_provider: &dyn ComponentFallbackProvider,
    view_state: &dyn SpaceViewState,
) {
    let name = A::name();
    if let Some(reflection) = ctx.reflection.archetypes.get(&name) {
        view_property_ui_impl(
            ctx,
            ui,
            view_id,
            name,
            reflection,
            view_state,
            fallback_provider,
        );
    } else {
        re_log::warn_once!("Missing reflection data for archetype {name:?}.");
    }
}

fn view_property_ui_impl(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_id: SpaceViewId,
    name: ArchetypeName,
    reflection: &ArchetypeReflection,
    view_state: &dyn SpaceViewState,
    fallback_provider: &dyn ComponentFallbackProvider,
) {
    let blueprint_path = entity_path_for_view_property(view_id, ctx.blueprint_db().tree(), name);
    let query_ctx = QueryContext {
        viewer_ctx: ctx,
        target_entity_path: &blueprint_path,
        archetype_name: Some(name),
        query: ctx.blueprint_query,
        view_state,
        view_ctx: None,
    };

    let component_results = ctx.blueprint_db().latest_at(
        ctx.blueprint_query,
        &blueprint_path,
        reflection.fields.iter().map(|field| field.component_name),
    );

    // If the property archetype only has a single component, don't show an additional hierarchy level!
    if reflection.fields.len() == 1 {
        let field = &reflection.fields[0];

        view_property_component_ui(
            &query_ctx,
            ui,
            field.component_name,
            reflection.display_name,
            name,
            field,
            &blueprint_path,
            component_results.get_or_empty(field.component_name),
            fallback_provider,
        );
    } else {
        let sub_prop_ui = |ui: &mut egui::Ui| {
            for field in &reflection.fields {
                let display_name = &field.display_name;

                view_property_component_ui(
                    &query_ctx,
                    ui,
                    field.component_name,
                    display_name,
                    name,
                    field,
                    &blueprint_path,
                    component_results.get_or_empty(field.component_name),
                    fallback_provider,
                );
            }
        };

        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(name.full_name()),
                true,
                list_item::LabelContent::new(reflection.display_name),
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
    archetype_name: ArchetypeName,
    field: &ArchetypeFieldReflection,
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
                    ctx.viewer_ctx.component_ui_registry.multiline_edit_ui(
                        ctx,
                        ui,
                        ctx.viewer_ctx.blueprint_db(),
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

    list_item_response = list_item_response.on_hover_ui(|ui| {
        let id = egui::Id::new((archetype_name, field.display_name));
        ui.markdown_ui(id, field.docstring_md);
    });

    view_property_context_menu(
        ctx.viewer_ctx,
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
            ctx.viewer_ctx
                .reset_blueprint_component_by_name(blueprint_path, component_name);
        })
        .value_fn(move |ui, _| {
            ctx.viewer_ctx.component_ui_registry.singleline_edit_ui(
                ctx,
                ui,
                ctx.viewer_ctx.blueprint_db(),
                blueprint_path,
                component_name,
                component_results,
                fallback_provider,
            );
        })
}
