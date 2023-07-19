use re_arrow_store::LatestAtQuery;
use re_log_types::{external::arrow2, DeserializableComponent, EntityPath, InstanceKey};
use re_query::ComponentWithInstances;
use re_viewer_context::{ComponentUiRegistry, UiVerbosity, ViewerContext};

use super::{DataUi, EntityDataUi};

pub fn create_component_ui_registry() -> ComponentUiRegistry {
    /// Registers how to show a given component in the ui.
    pub fn add<C: DeserializableComponent + EntityDataUi + re_types::Component>(
        registry: &mut ComponentUiRegistry,
    ) where
        for<'a> &'a C::ArrayType: IntoIterator,
    {
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

    // The things that are out-commented are components we have, but
    // where the default arrow-format for them looks good enough (at least for now).
    // Basically: adding custom UI:s for these out-commented components would be nice, but is not a must.
    add::<re_components::AnnotationContext>(&mut registry);
    // add::<re_components::Arrow3D>(&mut registry);
    // add::<re_components::Box3D>(&mut registry);
    add::<re_components::ClassId>(&mut registry);
    add::<re_components::ColorRGBA>(&mut registry);
    // add::<re_log_types::InstanceKey>(&mut registry);
    add::<re_components::KeypointId>(&mut registry);
    // add::<re_components::Label>(&mut registry);
    add::<re_components::LineStrip2D>(&mut registry);
    add::<re_components::LineStrip3D>(&mut registry);
    add::<re_components::Mesh3D>(&mut registry);
    // add::<re_components::Point2D>(&mut registry);
    // add::<re_components::Point3D>(&mut registry);
    add::<re_components::Pinhole>(&mut registry);
    // add::<re_components::Quaternion>(&mut registry);
    // add::<re_components::Radius>(&mut registry);
    add::<re_components::Rect2D>(&mut registry);
    // add::<re_components::Scalar>(&mut registry);
    // add::<re_components::ScalarPlotProps>(&mut registry);
    // add::<re_components::Size3D>(&mut registry);
    add::<re_components::Tensor>(&mut registry);
    add::<re_components::TextEntry>(&mut registry);
    add::<re_components::Transform3D>(&mut registry);
    add::<re_components::Vec2D>(&mut registry);
    add::<re_components::Vec3D>(&mut registry);
    add::<re_components::ViewCoordinates>(&mut registry);

    registry
}

fn fallback_component_ui(
    _ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    _verbosity: UiVerbosity,
    _query: &LatestAtQuery,
    _entity_path: &EntityPath,
    component: &ComponentWithInstances,
    instance_key: &InstanceKey,
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

impl DataUi for re_components::TextEntry {
    fn data_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        _query: &re_arrow_store::LatestAtQuery,
    ) {
        use re_viewer_context::level_to_rich_text;

        let Self { body, level } = self;

        match verbosity {
            UiVerbosity::Small => {
                ui.horizontal(|ui| {
                    if let Some(level) = level {
                        ui.label(level_to_rich_text(ui, level));
                    }
                    ui.label(format!("{body:?}")); // Debug format to get quotes and escapes
                });
            }
            UiVerbosity::All | UiVerbosity::Reduced => {
                egui::Grid::new("text_entry").num_columns(2).show(ui, |ui| {
                    ui.label("level:");
                    if let Some(level) = level {
                        ui.label(level_to_rich_text(ui, level));
                    }
                    ui.end_row();

                    ui.label("body:");
                    ui.label(format!("{body:?}")); // Debug format to get quotes and escapes
                    ui.end_row();
                });
            }
        }
    }
}

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
