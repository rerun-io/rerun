use re_arrow_store::LatestAtQuery;
use re_log_types::{
    component_types::InstanceKey, external::arrow2, DeserializableComponent, EntityPath, SizeBytes,
};
use re_query::ComponentWithInstances;
use re_viewer_context::{ComponentUiRegistry, UiVerbosity, ViewerContext};

use super::{DataUi, EntityDataUi};

pub fn create_component_ui_registry() -> ComponentUiRegistry {
    /// Registers how to show a given component in the ui.
    pub fn add<C: DeserializableComponent + EntityDataUi>(registry: &mut ComponentUiRegistry)
    where
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
    add::<re_log_types::component_types::AnnotationContext>(&mut registry);
    // add::<re_log_types::component_types::Arrow3D>(&mut registry);
    // add::<re_log_types::component_types::Box3D>(&mut registry);
    add::<re_log_types::component_types::ClassId>(&mut registry);
    add::<re_log_types::component_types::ColorRGBA>(&mut registry);
    // add::<re_log_types::component_types::InstanceKey>(&mut registry);
    add::<re_log_types::component_types::KeypointId>(&mut registry);
    // add::<re_log_types::component_types::Label>(&mut registry);
    add::<re_log_types::component_types::LineStrip2D>(&mut registry);
    add::<re_log_types::component_types::LineStrip3D>(&mut registry);
    add::<re_log_types::component_types::Mesh3D>(&mut registry);
    // add::<re_log_types::component_types::Point2D>(&mut registry);
    // add::<re_log_types::component_types::Point3D>(&mut registry);
    add::<re_log_types::component_types::Pinhole>(&mut registry);
    // add::<re_log_types::component_types::Quaternion>(&mut registry);
    // add::<re_log_types::component_types::Radius>(&mut registry);
    add::<re_log_types::component_types::Rect2D>(&mut registry);
    // add::<re_log_types::component_types::Scalar>(&mut registry);
    // add::<re_log_types::component_types::ScalarPlotProps>(&mut registry);
    // add::<re_log_types::component_types::Size3D>(&mut registry);
    add::<re_log_types::component_types::Tensor>(&mut registry);
    add::<re_log_types::component_types::TextEntry>(&mut registry);
    add::<re_log_types::component_types::Transform3D>(&mut registry);
    add::<re_log_types::component_types::Vec2D>(&mut registry);
    add::<re_log_types::component_types::Vec3D>(&mut registry);
    add::<re_log_types::ViewCoordinates>(&mut registry);

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
        let bytes = value.total_size_bytes();
        if bytes < 256 {
            // For small items, print them
            let mut repr = String::new();
            let display = arrow2::array::get_display(value.as_ref(), "null");
            display(&mut repr, 0).unwrap();
            ui.label(repr);
        } else {
            ui.label(format!("{bytes} bytes"));
        }
    } else {
        ui.weak("(null)");
    }
}

// ----------------------------------------------------------------------------

impl DataUi for re_log_types::component_types::TextEntry {
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

impl DataUi for re_log_types::component_types::Mesh3D {
    fn data_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &re_arrow_store::LatestAtQuery,
    ) {
        match self {
            re_log_types::Mesh3D::Encoded(mesh) => mesh.data_ui(ctx, ui, verbosity, query),
            re_log_types::Mesh3D::Raw(mesh) => mesh.data_ui(ctx, ui, verbosity, query),
        }
    }
}

impl DataUi for re_log_types::component_types::EncodedMesh3D {
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

impl DataUi for re_log_types::component_types::RawMesh3D {
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
