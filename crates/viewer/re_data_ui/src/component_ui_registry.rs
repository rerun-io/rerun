use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{external::arrow2, EntityPath};
use re_types::external::arrow2::array::Utf8Array;
use re_ui::UiExt;
use re_viewer_context::{ComponentUiRegistry, UiLayout, ViewerContext};

use super::EntityDataUi;

pub fn create_component_ui_registry() -> ComponentUiRegistry {
    re_tracing::profile_function!();

    let mut registry = ComponentUiRegistry::new(Box::new(&fallback_component_ui));

    add_to_registry::<re_types::components::AnnotationContext>(&mut registry);
    add_to_registry::<re_types::components::ClassId>(&mut registry);
    add_to_registry::<re_types::components::PinholeProjection>(&mut registry);
    add_to_registry::<re_types::components::KeypointId>(&mut registry);
    add_to_registry::<re_types::components::LineStrip2D>(&mut registry);
    add_to_registry::<re_types::components::LineStrip3D>(&mut registry);
    add_to_registry::<re_types::components::Resolution>(&mut registry);
    add_to_registry::<re_types::components::TensorData>(&mut registry);
    add_to_registry::<re_types::components::ViewCoordinates>(&mut registry);

    add_to_registry::<re_types_blueprint::blueprint::components::IncludedSpaceView>(&mut registry);
    add_to_registry::<re_types_blueprint::blueprint::components::SpaceViewMaximized>(&mut registry);
    add_to_registry::<re_types_blueprint::blueprint::components::VisualBounds2D>(&mut registry);

    registry
}

/// Registers how to show a given component in the ui.
pub fn add_to_registry<C: EntityDataUi + re_types::Component>(registry: &mut ComponentUiRegistry) {
    registry.add_display_ui(
        C::name(),
        Box::new(
            |ctx, ui, ui_layout, query, db, entity_path, component_raw| match C::from_arrow(
                component_raw,
            ) {
                Ok(components) => match components.len() {
                    0 => {
                        ui.weak("(empty)");
                    }
                    1 => {
                        components[0].entity_data_ui(ctx, ui, ui_layout, entity_path, query, db);
                    }
                    i => {
                        ui.label(format!("{} values", re_format::format_uint(i)));
                    }
                },
                Err(err) => {
                    ui.error_label("(failed to deserialize)")
                        .on_hover_text(err.to_string());
                }
            },
        ),
    );
}

fn fallback_component_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    _query: &LatestAtQuery,
    _db: &EntityDb,
    _entity_path: &EntityPath,
    component: &dyn arrow2::array::Array,
) {
    arrow_ui(ui, ui_layout, component);
}

fn arrow_ui(ui: &mut egui::Ui, ui_layout: UiLayout, array: &dyn arrow2::array::Array) {
    use re_types::SizeBytes as _;

    // Special-treat text.
    // Note: we match on the raw data here, so this works for any component containing text.
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i32>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            ui_layout.data_label(ui, string);
            return;
        }
    }
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i64>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            ui_layout.data_label(ui, string);
            return;
        }
    }

    let num_bytes = array.total_size_bytes();
    if num_bytes < 3000 {
        // Print small items:
        let mut string = String::new();
        let display = arrow2::array::get_display(array, "null");
        if display(&mut string, 0).is_ok() {
            ui_layout.data_label(ui, &string);
            return;
        }
    }

    // Fallback:
    let bytes = re_format::format_bytes(num_bytes as _);

    // TODO(emilk): pretty-print data type
    let data_type_formatted = format!("{:?}", array.data_type());

    if data_type_formatted.len() < 20 {
        // e.g. "4.2 KiB of Float32"
        ui_layout.data_label(ui, &format!("{bytes} of {data_type_formatted}"));
    } else {
        // Huge datatype, probably a union horror show
        ui.label(format!("{bytes} of data"));
    }
}
