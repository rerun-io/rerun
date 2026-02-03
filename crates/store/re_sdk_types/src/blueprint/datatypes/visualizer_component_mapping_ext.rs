use re_types_core::ArrowString;
use re_types_core::datatypes::Utf8;

use crate::blueprint::datatypes::{ComponentSourceKind, VisualizerComponentMapping};

impl VisualizerComponentMapping {
    /// Create a new visualizer component mapping.
    ///
    /// This is the most general constructor. For common cases, prefer the more specific constructors like
    /// [`Self::new_default`], [`Self::new_source_component`], or [`Self::new_override`].
    pub fn new(
        target: impl Into<Utf8>,
        source_kind: ComponentSourceKind,
        source_component: Option<impl Into<ArrowString>>,
        selector: Option<impl Into<ArrowString>>,
    ) -> Self {
        Self {
            target: target.into(),
            source_kind,
            source_component: source_component.map(|c| c.into()),
            selector: selector.map(|s| s.into()),
        }
    }

    /// Creates a new mapping that points to the view's default for the given target.
    ///
    /// # Example
    /// ```ignore
    /// // Use the view's default color instead of the entity's color
    /// VisualizerComponentMapping::new_default("SeriesLines:colors")
    /// ```
    pub fn new_default(target: impl Into<Utf8>) -> Self {
        Self {
            target: target.into(),
            source_kind: ComponentSourceKind::Default,
            source_component: None,
            selector: None,
        }
    }

    /// Creates a new mapping that sources from a specific component.
    ///
    /// # Example
    /// ```ignore
    /// // Source scalars from a custom component instead of the default
    /// VisualizerComponentMapping::new_source_component(
    ///     "Scalars:scalars",
    ///     "plot:my_custom_scalar",
    /// )
    /// ```
    pub fn new_source_component(
        target: impl Into<Utf8>,
        source_component: impl Into<ArrowString>,
    ) -> Self {
        Self {
            target: target.into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some(source_component.into()),
            selector: None,
        }
    }

    /// Creates a new mapping that sources from a specific component using a selector.
    ///
    /// ⚠️TODO(RR-3308): Not fully implemented yet.
    ///
    /// Selectors use jq-like syntax to pick a specific field within a component.
    ///
    /// # Example
    /// ```ignore
    /// // Pick just the x component from a vector
    /// VisualizerComponentMapping::new_source_component_with_selector("Scalar:scalar", "position", ".x")
    /// ```
    pub fn new_source_component_with_selector(
        target: impl Into<Utf8>,
        source_component: impl Into<ArrowString>,
        selector: impl Into<ArrowString>,
    ) -> Self {
        Self {
            target: target.into(),
            source_kind: ComponentSourceKind::SourceComponent,
            source_component: Some(source_component.into()),
            selector: Some(selector.into()),
        }
    }

    /// Creates a new mapping that uses an override value for the given target.
    ///
    /// This is typically not needed since in presence of an override,
    /// the override value will be used automatically unless specified otherwise.
    pub fn new_override(target: impl Into<Utf8>) -> Self {
        Self {
            target: target.into(),
            source_kind: ComponentSourceKind::Override,
            source_component: None,
            selector: None,
        }
    }
}
