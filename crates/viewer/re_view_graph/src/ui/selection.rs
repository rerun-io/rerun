use re_sdk_types::blueprint::components::Enabled;
use re_sdk_types::{Archetype, ArchetypeReflectionMarker, Component as _};
use re_view::{view_property_component_ui, view_property_component_ui_custom};
use re_viewer_context::ViewContext;
use re_viewport_blueprint::ViewProperty;

/// This function is similar to [`view_property_component_ui`], but it always
/// picks the [`Enabled`] component for the single-line edit UI.
/// Also, it will always show the single-line edit UI (and not only if there is
/// a single property per archetype).
pub fn view_property_force_ui<A: Archetype + ArchetypeReflectionMarker>(
    ctx: &ViewContext<'_>,
    ui: &mut egui::Ui,
) {
    let property =
        ViewProperty::from_archetype::<A>(ctx.blueprint_db(), ctx.blueprint_query(), ctx.view_id);

    let reflection = ctx.viewer_ctx.reflection();
    let Some(reflection) = reflection.archetypes.get(&property.archetype_name) else {
        // The `ArchetypeReflectionMarker` bound should make this impossible.
        re_log::warn_once!(
            "Missing reflection data for archetype {:?}.",
            property.archetype_name
        );
        return;
    };

    let query_ctx = property.query_context(ctx);

    if reflection.fields.len() == 1 {
        let field = &reflection.fields[0];

        view_property_component_ui(&query_ctx, ui, &property, reflection.display_name, field);
    } else {
        let sub_prop_ui = |ui: &mut egui::Ui| {
            for field in &reflection.fields {
                view_property_component_ui(&query_ctx, ui, &property, field.display_name, field);
            }
        };

        let field = reflection
            .fields
            .iter()
            .find(|field| field.component_type == Enabled::name())
            .expect("forces are required to have an `Enabled` component");

        let component_descr = field.component_descriptor(property.archetype_name);
        let component = component_descr.component;
        let component_array = property.component_raw(component);
        let row_id = property.component_row_id(component);

        let singleline_ui: &dyn Fn(&mut egui::Ui) = &|ui| {
            ctx.viewer_ctx.component_ui_registry().singleline_edit_ui(
                &query_ctx,
                ui,
                ctx.blueprint_db(),
                query_ctx.target_entity_path.clone(),
                &component_descr,
                row_id,
                component_array.as_deref(),
            );
        };

        view_property_component_ui_custom(
            &query_ctx,
            ui,
            &property,
            reflection.display_name,
            field,
            singleline_ui,
            Some(&sub_prop_ui),
        );
    }
}
