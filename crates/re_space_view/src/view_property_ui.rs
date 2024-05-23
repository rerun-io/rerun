use ahash::HashMap;
use re_types_core::Archetype;
use re_ui::list_item;
use re_viewer_context::{SpaceViewId, ViewerContext};
use re_viewport_blueprint::entity_path_for_view_property;

/// Display the UI for editing all components of a blueprint archetype.
///
/// Note that this will show default values for components that are null.
pub fn view_property_ui<A: Archetype>(
    ctx: &ViewerContext<'_>,
    space_view_id: SpaceViewId,
    ui: &mut egui::Ui,
) {
    let blueprint_db = ctx.store_context.blueprint;
    let blueprint_query = ctx.blueprint_query;
    let blueprint_path = entity_path_for_view_property::<A>(space_view_id, blueprint_db.tree());

    let component_names = A::all_components();
    let component_results = blueprint_db.latest_at(
        blueprint_query,
        &blueprint_path,
        component_names.iter().copied(),
    );

    let field_info_per_component: HashMap<_, _> = A::field_infos()
        .map(|field_infos| {
            field_infos
                .iter()
                .cloned()
                .map(|field_info| (field_info.component_name, field_info))
                .collect()
        })
        .unwrap_or_default();

    let sub_prop_ui = |re_ui: &re_ui::ReUi, ui: &mut egui::Ui| {
        for component_name in component_names.as_ref() {
            if component_name.is_indicator_component() {
                continue;
            }

            let field_info = field_info_per_component.get(component_name);
            let display_name =
                field_info.map_or_else(|| component_name.short_name(), |info| info.display_name);

            let list_item_response = list_item::ListItem::new(re_ui)
                .interactive(false)
                .show_flat(
                    ui,
                    list_item::PropertyContent::new(display_name)
                        .action_button(&re_ui::icons::RESET, || {
                            ctx.reset_blueprint_component_by_name(&blueprint_path, *component_name);
                        })
                        .value_fn(|_, ui, _| {
                            ctx.component_ui_registry.edit_ui(
                                ctx,
                                ui,
                                re_viewer_context::UiLayout::List,
                                blueprint_query,
                                blueprint_db,
                                &blueprint_path,
                                &blueprint_path,
                                component_results.get(*component_name),
                                *component_name,
                                &0.into(),
                            );
                        }),
                );

            let list_item_response =
                if let Some(tooltip) = field_info.map(|info| info.documentation) {
                    list_item_response.on_hover_text(tooltip)
                } else {
                    list_item_response
                };

            list_item_response.context_menu(|ui| {
                if ui.button("Reset to default blueprint.")
                     .on_hover_text("Resets this property to the value in the default blueprint.\n
If no default blueprint was set or it didn't set any value for this field, this is the same as resetting to empty.")
                     .clicked() {
                    ctx.reset_blueprint_component_by_name(&blueprint_path, *component_name);
                    ui.close_menu();
                }
                ui.add_enabled_ui(component_results.contains_non_empty(*component_name), |ui| {
                    if ui.button("Reset to empty.")
                        .on_hover_text("Resets this property to an unset value, meaning that a heuristically determined value will be used instead.\n
This has the same effect as not setting the value in the blueprint at all.")
                        .on_disabled_hover_text("The property is already unset.")
                        .clicked() {
                        ctx.save_empty_blueprint_component_by_name(&blueprint_path, *component_name);
                        ui.close_menu();
                    }
                });

                // TODO(andreas): The next logical thing here is now to save it to the default blueprint!
                // This should be fairly straight forward except that we need to make sure that a default blueprint exists in the first place.
            });
        }
    };

    list_item::ListItem::new(ctx.re_ui)
        .interactive(false)
        .show_hierarchical_with_children(
            ui,
            A::name().full_name(),
            true,
            list_item::LabelContent::new(A::display_name()),
            sub_prop_ui,
        );
}
