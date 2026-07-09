//! View types for blueprint configuration.

use std::collections::HashMap;
use uuid::Uuid;

use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes::{
    ActiveVisualizers, ViewBlueprint, ViewContents, VisualizerInstruction,
};
use re_sdk_types::blueprint::components::{QueryExpression, ViewClass};
use re_sdk_types::components::{Name, Visible};
use re_sdk_types::datatypes::Bool;
use re_sdk_types::{AsComponents, SerializedComponentBatch, Visualizer};

macro_rules! impl_view_common_methods {
    () => {
        /// Set the origin entity path.
        pub fn with_origin(mut self, origin: impl Into<EntityPath>) -> Self {
            self.0.origin = origin.into();
            self
        }

        /// Set the contents query expressions.
        pub fn with_contents(
            mut self,
            queries: impl IntoIterator<Item = impl Into<String>>,
        ) -> Self {
            self.0.contents = queries.into_iter().map(Into::into).collect();
            self
        }

        /// Set visibility.
        pub fn with_visible(mut self, visible: bool) -> Self {
            self.0.visible = Some(visible);
            self
        }

        /// Add a default archetype that applies to all entities in the view.
        pub fn with_defaults(mut self, archetype: &dyn AsComponents) -> Self {
            self.0.add_defaults(archetype);
            self
        }

        /// Add a visualizer override for a specific entity.
        pub fn with_override(
            self,
            entity_path: impl Into<EntityPath>,
            visualizers: impl Into<Visualizer>,
        ) -> Self {
            self.with_overrides(entity_path, [visualizers])
        }

        /// Add visualizer overrides for a specific entity.
        pub fn with_overrides(
            mut self,
            entity_path: impl Into<EntityPath>,
            visualizers: impl IntoIterator<Item = impl Into<Visualizer>>,
        ) -> Self {
            self.0.add_overrides(entity_path, visualizers);
            self
        }
    };
}

/// Converts a value into a blueprint property archetype for a view.
///
/// This mirrors Python's view constructor aliases while keeping the Rust view
/// setters strongly typed.
pub trait IntoViewProperty<T> {
    /// Convert into the archetype stored as the view property.
    fn into_view_property(self) -> T;
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::Background>
    for re_sdk_types::blueprint::components::BackgroundKind
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::Background {
        re_sdk_types::blueprint::archetypes::Background::new(self)
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::Background>
    for &re_sdk_types::blueprint::components::BackgroundKind
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::Background {
        re_sdk_types::blueprint::archetypes::Background::new(self.clone())
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::Background>
    for re_sdk_types::components::Color
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::Background {
        re_sdk_types::blueprint::archetypes::Background::new(
            re_sdk_types::blueprint::components::BackgroundKind::SolidColor,
        )
        .with_color(self)
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::Background>
    for &re_sdk_types::components::Color
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::Background {
        self.clone().into_view_property()
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::LineGrid3D> for bool {
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::LineGrid3D {
        re_sdk_types::blueprint::archetypes::LineGrid3D::new().with_visible(self)
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::MapBackground>
    for re_sdk_types::blueprint::components::MapProvider
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::MapBackground {
        re_sdk_types::blueprint::archetypes::MapBackground::new(self)
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::MapBackground>
    for &re_sdk_types::blueprint::components::MapProvider
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::MapBackground {
        re_sdk_types::blueprint::archetypes::MapBackground::new(self.clone())
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::MapZoom>
    for re_sdk_types::blueprint::components::ZoomLevel
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::MapZoom {
        re_sdk_types::blueprint::archetypes::MapZoom::new(self)
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::MapZoom>
    for &re_sdk_types::blueprint::components::ZoomLevel
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::MapZoom {
        re_sdk_types::blueprint::archetypes::MapZoom::new(self.clone())
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::MapZoom> for f64 {
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::MapZoom {
        re_sdk_types::blueprint::archetypes::MapZoom::new(self)
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::PlotLegend>
    for re_sdk_types::blueprint::components::Corner2D
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::PlotLegend {
        re_sdk_types::blueprint::archetypes::PlotLegend::new().with_corner(self)
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::PlotLegend>
    for &re_sdk_types::blueprint::components::Corner2D
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::PlotLegend {
        re_sdk_types::blueprint::archetypes::PlotLegend::new().with_corner(self.clone())
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::TensorViewFit>
    for re_sdk_types::blueprint::components::ViewFit
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::TensorViewFit {
        re_sdk_types::blueprint::archetypes::TensorViewFit::new().with_scaling(self)
    }
}

impl IntoViewProperty<re_sdk_types::blueprint::archetypes::TensorViewFit>
    for &re_sdk_types::blueprint::components::ViewFit
{
    fn into_view_property(self) -> re_sdk_types::blueprint::archetypes::TensorViewFit {
        re_sdk_types::blueprint::archetypes::TensorViewFit::new().with_scaling(self.clone())
    }
}

/// A view in the blueprint.
#[derive(Debug)]
pub struct View {
    pub(crate) id: Uuid,
    pub(crate) class_identifier: String,
    pub(crate) name: Option<String>,
    pub(crate) origin: EntityPath,
    pub(crate) contents: Vec<String>,
    pub(crate) visible: Option<bool>,
    pub(crate) properties: HashMap<String, Vec<SerializedComponentBatch>>,
    pub(crate) defaults: Vec<Vec<SerializedComponentBatch>>,
    pub(crate) overrides: HashMap<EntityPath, Vec<Visualizer>>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            class_identifier: String::new(),
            name: None,
            origin: "/".into(),
            contents: vec!["$origin/**".into()],
            visible: None,
            properties: HashMap::new(),
            defaults: Vec::new(),
            overrides: HashMap::new(),
        }
    }
}

impl View {
    /// Create a new view wrapper for a registered view class.
    pub(crate) fn new(class_identifier: impl Into<String>, name: Option<String>) -> Self {
        Self {
            class_identifier: class_identifier.into(),
            name,
            ..Default::default()
        }
    }

    /// Get the blueprint path for this view.
    pub fn blueprint_path(&self) -> EntityPath {
        format!("view/{}", self.id).into()
    }

    /// Add a property archetype that applies to the view itself.
    pub(crate) fn add_property(&mut self, name: &str, archetype: &dyn AsComponents) {
        self.properties
            .insert(name.to_owned(), archetype.as_serialized_batches());
    }

    /// Add a default archetype that applies to all entities in the view.
    pub(crate) fn add_defaults(&mut self, archetype: &dyn AsComponents) {
        self.defaults.push(archetype.as_serialized_batches());
    }

    /// Add visualizer overrides for a specific entity.
    pub(crate) fn add_overrides(
        &mut self,
        entity_path: impl Into<EntityPath>,
        visualizers: impl IntoIterator<Item = impl Into<Visualizer>>,
    ) {
        self.overrides
            .entry(entity_path.into())
            .or_default()
            .extend(visualizers.into_iter().map(Into::into));
    }

    /// Log this view to the blueprint stream.
    pub(crate) fn log_to_stream(
        &self,
        stream: &crate::RecordingStream,
    ) -> crate::RecordingStreamResult<()> {
        let view_contents = ViewContents::new(
            self.contents
                .iter()
                .map(|q| QueryExpression(q.clone().into())),
        );

        stream.log(
            format!("{}/ViewContents", self.blueprint_path()),
            &view_contents,
        )?;

        let mut arch = ViewBlueprint::new(ViewClass(self.class_identifier.clone().into()));

        if let Some(ref name) = self.name {
            arch = arch.with_display_name(Name(name.clone().into()));
        }

        arch = arch.with_space_origin(self.origin.to_string());

        if let Some(visible) = self.visible {
            arch = arch.with_visible(Visible(Bool(visible)));
        }

        stream.log(self.blueprint_path(), &arch)?;

        // Log view-specific properties/settings
        for (prop_name, prop_batches) in &self.properties {
            stream.log_serialized_batches(
                format!("{}/{}", self.blueprint_path(), prop_name),
                false,
                prop_batches.iter().cloned(),
            )?;
        }

        // Log defaults
        for default_batches in &self.defaults {
            stream.log_serialized_batches(
                format!("{}/defaults", self.blueprint_path()),
                false,
                default_batches.iter().cloned(),
            )?;
        }

        // Log overrides
        for (entity_path, visualizers) in &self.overrides {
            let base_visualizer_path =
                ViewContents::blueprint_base_visualizer_path_for_entity(self.id, entity_path);

            let mut visualizer_ids = Vec::new();

            for visualizer in visualizers {
                // Log the visualizer instruction (which contains the visualizer type)
                let visualizer_path = base_visualizer_path
                    .join(&EntityPath::from_single_string(visualizer.id.0.to_string()));

                let mut instruction =
                    VisualizerInstruction::new(visualizer.visualizer_type.clone());
                if !visualizer.mappings.is_empty() {
                    instruction = instruction.with_component_map(visualizer.mappings.clone());
                }
                stream.log(visualizer_path.clone(), &instruction)?;

                // Log the overrides if any
                if !visualizer.overrides.is_empty() {
                    stream.log_serialized_batches(
                        visualizer_path,
                        false,
                        visualizer.overrides.iter().cloned(),
                    )?;
                }

                visualizer_ids.push(visualizer.id);
            }

            // Log the active visualizers list
            if !visualizer_ids.is_empty() {
                stream.log(
                    base_visualizer_path,
                    &ActiveVisualizers::new(visualizer_ids),
                )?;
            }
        }

        Ok(())
    }
}

include!("generated_views.rs");
