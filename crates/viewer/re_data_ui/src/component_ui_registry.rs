use re_ui::UiExt;
use re_viewer_context::ComponentUiRegistry;

use super::EntityDataUi;

pub fn register_component_uis(registry: &mut re_viewer_context::ComponentUiRegistry) {
    re_tracing::profile_function!();

    // TODO(#6661): Move this to component_ui_registry. Separate components could simplify this to the extent that multi/single line is enough?
    add_to_registry::<re_types::components::AnnotationContext>(registry);

    // TODO(#6661): Move this to component_ui_registry. Image preview is a bit hard because row_id and size stuff needs to be known. `ImageBuffer` needs to be handled as well.
    add_to_registry::<re_types::components::Blob>(registry);
    add_to_registry::<re_types::components::TensorData>(registry);

    // TODO(#6661): Move this to component_ui_registry. Funky AnnotationContext querying thing. Maybe we can get away with a store querying hack?
    add_to_registry::<re_types::components::ClassId>(registry);
    add_to_registry::<re_types::components::KeypointId>(registry);
}

/// Registers how to show a given component in the ui.
pub fn add_to_registry<C: EntityDataUi + re_types::Component>(registry: &mut ComponentUiRegistry) {
    registry.add_legacy_display_ui(
        C::name(),
        Box::new(
            |ctx, ui, ui_layout, query, db, entity_path, row_id, component_raw| match C::from_arrow(
                component_raw,
            ) {
                Ok(components) => match components.len() {
                    0 => {
                        ui.weak("(empty)");
                    }
                    1 => {
                        components[0].entity_data_ui(
                            ctx,
                            ui,
                            ui_layout,
                            entity_path,
                            row_id,
                            query,
                            db,
                        );
                    }
                    i => {
                        ui.label(format!("{} values", re_format::format_uint(i)));
                    }
                },
                Err(err) => {
                    ui.error_with_details_on_hover("(failed to deserialize)")
                        .on_hover_text(err.to_string());
                }
            },
        ),
    );
}
