use re_chunk_store::{external::re_chunk::Arrow2Array, RowId};
use re_types_core::{
    reflection::{ArchetypeFieldReflection, ArchetypeReflection},
    Archetype, ArchetypeName, ArchetypeReflectionMarker, ComponentName,
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
pub fn view_property_ui<A: Archetype + ArchetypeReflectionMarker>(
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
        // The `ArchetypeReflectionMarker` bound should make this impossible.
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

        let component_array = component_results.component_batch_raw(&field.component_name);
        view_property_component_ui(
            &query_ctx,
            ui,
            field.component_name,
            reflection.display_name,
            field,
            &blueprint_path,
            component_results.component_row_id(&field.component_name),
            component_array.as_deref(),
            fallback_provider,
        );
    } else {
        let sub_prop_ui = |ui: &mut egui::Ui| {
            for field in &reflection.fields {
                let display_name = &field.display_name;

                let component_array = component_results.component_batch_raw(&field.component_name);
                view_property_component_ui(
                    &query_ctx,
                    ui,
                    field.component_name,
                    display_name,
                    field,
                    &blueprint_path,
                    component_results.component_row_id(&field.component_name),
                    component_array.as_deref(),
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
    field: &ArchetypeFieldReflection,
    blueprint_path: &re_log_types::EntityPath,
    row_id: Option<RowId>,
    component_array: Option<&dyn Arrow2Array>,
    fallback_provider: &dyn ComponentFallbackProvider,
) {
    let singleline_list_item_content = singleline_list_item_content(
        ctx,
        root_item_display_name,
        blueprint_path,
        component_name,
        row_id,
        component_array,
        fallback_provider,
    );

    let ui_types = ctx
        .viewer_ctx
        .component_ui_registry
        .registered_ui_types(component_name);

    let list_item_response = if ui_types.contains(ComponentUiTypes::MultiLineEditor) {
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
                        row_id,
                        component_array,
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

    list_item_response.on_hover_ui(|ui| {
        ui.markdown_ui(field.docstring_md);
    });
}

fn menu_more(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    blueprint_path: &re_log_types::EntityPath,
    component_name: ComponentName,
    component_array: Option<&dyn Arrow2Array>,
) {
    let property_differs_from_default = component_array
        != ctx
            .raw_latest_at_in_default_blueprint(blueprint_path, component_name)
            .as_deref();

    let response = ui
        .add_enabled(
            property_differs_from_default,
            egui::Button::new("Reset to default blueprint"),
        )
        .on_hover_text(
"Resets this property to the value in the default blueprint.
If no default blueprint was set or it didn't set any value for this field, this is the same as resetting to empty."
        )
        .on_disabled_hover_text(
            "The property is already set to the same value it has in the default blueprint",
        );
    if response.clicked() {
        ctx.reset_blueprint_component_by_name(blueprint_path, component_name);
        ui.close_menu();
    }

    let response = ui
        .add_enabled(
            component_array.is_some(),
            egui::Button::new("Unset"),
        )
        .on_hover_text(
"Resets this property to an unset value, meaning that a heuristically determined value will be used instead.
This has the same effect as not setting the value in the blueprint at all."
        )
        .on_disabled_hover_text("The property is already unset.");
    if response.clicked() {
        ctx.clear_blueprint_component_by_name(blueprint_path, component_name);
        ui.close_menu();
    }

    // TODO(andreas): The next logical thing here is now to save it to the default blueprint!
    // This should be fairly straight forward except that we need to make sure that a default blueprint exists in the first place.
}

fn singleline_list_item_content<'a>(
    ctx: &'a QueryContext<'_>,
    display_name: &str,
    blueprint_path: &'a re_log_types::EntityPath,
    component_name: ComponentName,
    row_id: Option<RowId>,
    component_array: Option<&'a dyn Arrow2Array>,
    fallback_provider: &'a dyn ComponentFallbackProvider,
) -> list_item::PropertyContent<'a> {
    list_item::PropertyContent::new(display_name)
        .menu_button(&re_ui::icons::MORE, move |ui| {
            menu_more(
                ctx.viewer_ctx,
                ui,
                blueprint_path,
                component_name,
                component_array,
            );
        })
        .value_fn(move |ui, _| {
            ctx.viewer_ctx.component_ui_registry.singleline_edit_ui(
                ctx,
                ui,
                ctx.viewer_ctx.blueprint_db(),
                blueprint_path,
                component_name,
                row_id,
                component_array,
                fallback_provider,
            );
        })
}
