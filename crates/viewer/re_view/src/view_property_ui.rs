use re_types::ComponentDescriptor;
use re_types_core::{reflection::ArchetypeFieldReflection, Archetype, ArchetypeReflectionMarker};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::{
    ComponentFallbackProvider, ComponentUiTypes, QueryContext, ViewId, ViewState, ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

/// Display the UI for editing all components of a blueprint archetype.
///
/// Note that this will show default values for components that are null.
pub fn view_property_ui<A: Archetype + ArchetypeReflectionMarker>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_id: ViewId,
    fallback_provider: &dyn ComponentFallbackProvider,
    view_state: &dyn ViewState,
) {
    let view_property =
        ViewProperty::from_archetype::<A>(ctx.blueprint_db(), ctx.blueprint_query, view_id);
    view_property_ui_impl(ctx, ui, &view_property, fallback_provider, view_state);
}

fn view_property_ui_impl(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    property: &ViewProperty,
    fallback_provider: &dyn ComponentFallbackProvider,
    view_state: &dyn ViewState,
) {
    let Some(reflection) = ctx.reflection().archetypes.get(&property.archetype_name) else {
        // The `ArchetypeReflectionMarker` bound should make this impossible.
        re_log::warn_once!(
            "Missing reflection data for archetype {:?}.",
            property.archetype_name
        );
        return;
    };

    let query_ctx = property.query_context(ctx, view_state);
    // If the property archetype only has a single component, don't show an additional hierarchy level!
    if reflection.fields.len() == 1 {
        let field = &reflection.fields[0];

        view_property_component_ui(
            &query_ctx,
            ui,
            property,
            reflection.display_name,
            field,
            fallback_provider,
        );
    } else {
        let sub_prop_ui = |ui: &mut egui::Ui| {
            for field in &reflection.fields {
                view_property_component_ui(
                    &query_ctx,
                    ui,
                    property,
                    field.display_name,
                    field,
                    fallback_provider,
                );
            }
        };

        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(property.archetype_name.full_name()),
                true,
                list_item::LabelContent::new(reflection.display_name),
                sub_prop_ui,
            );
    }
}

/// Draw view property ui for a single component of a view property archetype.
///
/// Use [`view_property_ui`] whenever possible to show the ui for all components of a view property archetype.
/// This function is only useful if you want to show custom ui for some of the components.
pub fn view_property_component_ui(
    ctx: &QueryContext<'_>,
    ui: &mut egui::Ui,
    property: &ViewProperty,
    display_name: &str,
    field: &ArchetypeFieldReflection,
    fallback_provider: &dyn ComponentFallbackProvider,
) {
    let component_descr = field.component_descriptor(property.archetype_name);

    let component_array = property.component_raw(&component_descr);
    let row_id = property.component_row_id(field.component_name);

    let ui_types = ctx
        .viewer_ctx
        .component_ui_registry()
        .registered_ui_types(field.component_name);

    let singleline_ui: &dyn Fn(&mut egui::Ui) = &|ui| {
        ctx.viewer_ctx.component_ui_registry().singleline_edit_ui(
            ctx,
            ui,
            ctx.viewer_ctx.blueprint_db(),
            ctx.target_entity_path,
            &component_descr,
            row_id,
            component_array.as_deref(),
            fallback_provider,
        );
    };

    let multiline_ui: &dyn Fn(&mut egui::Ui) = &|ui| {
        ctx.viewer_ctx.component_ui_registry().multiline_edit_ui(
            ctx,
            ui,
            ctx.viewer_ctx.blueprint_db(),
            ctx.target_entity_path,
            &component_descr,
            row_id,
            component_array.as_deref(),
            fallback_provider,
        );
    };
    // Do this as a separate step to avoid borrowing issues.
    let multiline_ui_ref: Option<&dyn Fn(&mut egui::Ui)> =
        if ui_types.contains(ComponentUiTypes::MultiLineEditor) {
            Some(multiline_ui)
        } else {
            None
        };

    view_property_component_ui_custom(
        ctx,
        ui,
        property,
        display_name,
        field,
        singleline_ui,
        multiline_ui_ref,
    );
}

/// Draw view property ui for a single component of a view property archetype with custom ui for singleline & multiline.
///
/// Use [`view_property_ui`] whenever possible to show the ui for all components of a view property archetype.
/// This function is only useful if you want to show custom ui for some of the components.
pub fn view_property_component_ui_custom(
    ctx: &QueryContext<'_>,
    ui: &mut egui::Ui,
    property: &ViewProperty,
    display_name: &str,
    field: &ArchetypeFieldReflection,
    singleline_ui: &dyn Fn(&mut egui::Ui),
    multiline_ui: Option<&dyn Fn(&mut egui::Ui)>,
) {
    let component_descr = field.component_descriptor(property.archetype_name);

    let singleline_list_item_content = list_item::PropertyContent::new(display_name)
        .menu_button(&re_ui::icons::MORE, |ui| {
            menu_more(ctx.viewer_ctx, ui, property, &component_descr);
        })
        .value_fn(move |ui, _| singleline_ui(ui));

    let list_item_response = if let Some(multiline_ui) = multiline_ui {
        let default_open = false;
        let id = egui::Id::new((ctx.target_entity_path.hash(), component_descr.full_name()));
        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                id,
                default_open,
                singleline_list_item_content,
                |ui| {
                    multiline_ui(ui);
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
    property: &ViewProperty,
    component_descr: &ComponentDescriptor,
) {
    let component_array = property.component_raw(component_descr);

    let property_differs_from_default = component_array
        != ctx.raw_latest_at_in_default_blueprint(&property.blueprint_store_path, component_descr);

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
        ctx.reset_blueprint_component(&property.blueprint_store_path, component_descr.clone());
        ui.close();
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
        ctx.clear_blueprint_component(&property.blueprint_store_path, component_descr.clone());
        ui.close();
    }

    // TODO(andreas): The next logical thing here is now to save it to the default blueprint!
    // This should be fairly straight forward except that we need to make sure that a default blueprint exists in the first place.
}
