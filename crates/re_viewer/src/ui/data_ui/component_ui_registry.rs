use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_log_types::{
    component_types::InstanceKey,
    external::arrow2,
    external::arrow2_convert::{
        deserialize::{ArrowArray, ArrowDeserialize},
        field::ArrowField,
    },
    msg_bundle::Component,
    ComponentName,
};
use re_query::ComponentWithInstances;

use crate::{misc::ViewerContext, ui::UiVerbosity};

use super::DataUi;

type ComponentUiCallback = Box<
    dyn Fn(
        &mut ViewerContext<'_>,
        &mut egui::Ui,
        UiVerbosity,
        &LatestAtQuery,
        &ComponentWithInstances,
        &InstanceKey,
    ),
>;

/// How to display components in a Ui
pub struct ComponentUiRegistry {
    components: BTreeMap<ComponentName, ComponentUiCallback>,
}

impl Default for ComponentUiRegistry {
    fn default() -> Self {
        let mut registry = Self {
            components: Default::default(),
        };

        // The things that are out-commented are components we have, but
        // where the default arrow-format for them looks good enough (at least for now).
        // Basically: adding custom UI:s for these out-commented components would be nice, but is not a must.
        registry.add::<re_log_types::component_types::AnnotationContext>();
        // registry.add::<re_log_types::component_types::Arrow3D>();
        // registry.add::<re_log_types::component_types::Box3D>();
        // registry.add::<re_log_types::component_types::ClassId>();
        registry.add::<re_log_types::component_types::ColorRGBA>();
        // registry.add::<re_log_types::component_types::InstanceKey>();
        // registry.add::<re_log_types::component_types::KeypointId>();
        // registry.add::<re_log_types::component_types::Label>();
        registry.add::<re_log_types::component_types::LineStrip2D>();
        registry.add::<re_log_types::component_types::LineStrip3D>();
        registry.add::<re_log_types::component_types::Mesh3D>();
        registry.add::<re_log_types::component_types::MsgId>();
        // registry.add::<re_log_types::component_types::Point2D>();
        // registry.add::<re_log_types::component_types::Point3D>();
        // registry.add::<re_log_types::component_types::Quaternion>();
        // registry.add::<re_log_types::component_types::Radius>();
        registry.add::<re_log_types::component_types::Rect2D>();
        // registry.add::<re_log_types::component_types::Scalar>();
        // registry.add::<re_log_types::component_types::ScalarPlotProps>();
        // registry.add::<re_log_types::component_types::Size3D>();
        registry.add::<re_log_types::component_types::Tensor>();
        registry.add::<re_log_types::component_types::TextEntry>();
        registry.add::<re_log_types::component_types::Transform>();
        registry.add::<re_log_types::component_types::Vec2D>();
        registry.add::<re_log_types::component_types::Vec3D>();
        registry.add::<re_log_types::ViewCoordinates>();

        registry
    }
}

impl ComponentUiRegistry {
    fn add<C>(&mut self)
    where
        C: Component + DataUi + ArrowDeserialize + ArrowField<Type = C> + 'static,
        C::ArrayType: ArrowArray,
        for<'a> &'a C::ArrayType: IntoIterator,
    {
        self.components.insert(
            C::name(),
            Box::new(|ctx, ui, verbosity, query, component, instance| {
                match component.lookup::<C>(instance) {
                    Ok(component) => component.data_ui(ctx, ui, verbosity, query),
                    Err(re_query::QueryError::ComponentNotFound) => {
                        ui.weak("(not found)");
                    }
                    Err(err) => {
                        re_log::warn_once!("Expected component {}, {}", C::name(), err);
                    }
                }
            }),
        );
    }

    /// Show a ui for this instance of this component.
    pub fn ui(
        &self,
        ctx: &mut crate::misc::ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: crate::ui::UiVerbosity,
        query: &LatestAtQuery,
        component: &ComponentWithInstances,
        instance_key: &InstanceKey,
    ) {
        if component.name() == InstanceKey::name() {
            // The user wants to show a ui for the `InstanceKey` component - well, that's easy:
            ui.label(instance_key.to_string());
            return;
        }

        if let Some(ui_callback) = self.components.get(&component.name()) {
            (*ui_callback)(ctx, ui, verbosity, query, component, instance_key);
        } else {
            // No special ui implementation - use a generic one:
            if let Some(value) = component.lookup_arrow(instance_key) {
                let bytes = arrow2::compute::aggregate::estimated_bytes_size(value.as_ref());
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
        use crate::ui::view_text::level_to_rich_text;

        let Self { body, level } = self;

        match verbosity {
            UiVerbosity::Small | UiVerbosity::MaxHeight(_) => {
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
