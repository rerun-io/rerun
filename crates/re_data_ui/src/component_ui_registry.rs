use re_arrow_store::LatestAtQuery;
use re_log_types::{external::arrow2, EntityPath};
use re_query::ComponentWithInstances;
use re_viewer_context::{ComponentUiRegistry, UiVerbosity, ViewerContext};

use super::{DataUi, EntityDataUi};

pub fn create_component_ui_registry() -> ComponentUiRegistry {
    /// Registers how to show a given component in the ui.
    pub fn add<C: EntityDataUi + re_types::Component>(registry: &mut ComponentUiRegistry) {
        registry.add(
            C::name(),
            Box::new(
                |ctx, ui, verbosity, query, entity_path, component, instance| match component
                    .lookup::<C>(instance)
                {
                    Ok(component) => {
                        component.entity_data_ui(ctx, ui, verbosity, entity_path, query);
                    }
                    Err(re_query::QueryError::ComponentNotFound) => {
                        ui.weak("(not found)");
                    }
                    Err(err) => {
                        re_log::warn_once!("Expected component {}, {}", C::name(), err);
                    }
                },
            ),
        );
    }

    let mut registry = ComponentUiRegistry::new(Box::new(&fallback_component_ui));

    add::<re_components::Mesh3D>(&mut registry);
    add::<re_components::Pinhole>(&mut registry);
    add::<re_components::ViewCoordinates>(&mut registry);
    add::<re_types::components::AnnotationContext>(&mut registry);
    add::<re_types::components::ClassId>(&mut registry);
    add::<re_types::components::Color>(&mut registry);
    add::<re_types::components::KeypointId>(&mut registry);
    add::<re_types::components::Transform3D>(&mut registry);
    add::<re_types::components::LineStrip2D>(&mut registry);
    add::<re_types::components::LineStrip3D>(&mut registry);
    add::<re_types::components::TensorData>(&mut registry);

    registry
}

fn fallback_component_ui(
    _ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    _verbosity: UiVerbosity,
    _query: &LatestAtQuery,
    _entity_path: &EntityPath,
    component: &ComponentWithInstances,
    instance_key: &re_types::components::InstanceKey,
) {
    // No special ui implementation - use a generic one:
    if let Some(value) = component.lookup_arrow(instance_key) {
        ui.label(format_arrow(&*value));
    } else {
        ui.weak("(null)");
    }
}

fn format_arrow(value: &dyn arrow2::array::Array) -> String {
    use re_log_types::SizeBytes as _;

    let bytes = value.total_size_bytes();
    if bytes < 256 {
        // Print small items:
        let mut string = String::new();
        let display = arrow2::array::get_display(value, "null");
        if display(&mut string, 0).is_ok() {
            return string;
        }
    }

    // Fallback:
    format!("{bytes} bytes")
}

// ----------------------------------------------------------------------------

impl DataUi for re_components::Mesh3D {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            re_components::Mesh3D::Encoded(mesh) => mesh.data_ui(ctx, ui, verbosity, query),
            re_components::Mesh3D::Raw(mesh) => mesh.data_ui(ctx, ui, verbosity, query),
        }
    }
}

impl DataUi for re_components::EncodedMesh3D {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.label(format!("{} mesh", self.format));
    }
}

impl DataUi for re_components::RawMesh3D {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        ui.label(format!(
            "mesh ({} triangles)",
            re_format::format_number(self.num_triangles())
        ));
    }
}
