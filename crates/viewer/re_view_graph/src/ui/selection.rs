use re_log_types::hash::Hash64;
use re_types::{
    blueprint::components::Enabled, Archetype, ArchetypeReflectionMarker, Component as _,
    ComponentDescriptor,
};
use re_view::{view_property_component_ui, view_property_component_ui_custom};
use re_viewer_context::{ComponentFallbackProvider, ViewId, ViewState, ViewerContext};
use re_viewport_blueprint::ViewProperty;

/// This function is similar to [`view_property_component_ui`], but it always
/// picks the [`Enabled`] component for the single-line edit UI.
/// Also, it will always show the single-line edit UI (and not only if there is
/// a single property per archetype).
pub fn view_property_force_ui<A: Archetype + ArchetypeReflectionMarker>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_id: ViewId,
    fallback_provider: &dyn ComponentFallbackProvider,
    view_state: &dyn ViewState,
) {
    let property =
        ViewProperty::from_archetype::<A>(ctx.blueprint_db(), ctx.blueprint_query, view_id);

    let Some(reflection) = ctx.reflection().archetypes.get(&property.archetype_name) else {
        // The `ArchetypeReflectionMarker` bound should make this impossible.
        re_log::warn_once!(
            "Missing reflection data for archetype {:?}.",
            property.archetype_name
        );
        return;
    };

    let query_ctx = property.query_context(ctx, view_state);

    if reflection.fields.len() == 1 {
        let field = &reflection.fields[0];

        view_property_component_ui(
            &query_ctx,
            ui,
            &property,
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
                    &property,
                    field.display_name,
                    field,
                    fallback_provider,
                );
            }
        };

        let field = reflection
            .fields
            .iter()
            .find(|field| field.component_name == Enabled::name())
            .expect("forces are required to have an `Enabled` component");

        let component_descr = ComponentDescriptor {
            archetype_name: Some(reflection.display_name.into()),
            archetype_field_name: Some(field.name.into()),
            component_name: field.component_name,
        };

        let component_array = property.component_raw(&component_descr);
        let row_id = property.component_row_id(field.component_name);

        let singleline_ui: &dyn Fn(&mut egui::Ui) = &|ui| {
            ctx.component_ui_registry().singleline_edit_ui(
                &query_ctx,
                ui,
                ctx.blueprint_db(),
                query_ctx.target_entity_path,
                field.component_name,
                row_id.map(Hash64::hash),
                component_array.as_deref(),
                fallback_provider,
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
