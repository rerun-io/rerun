use re_chunk::{ComponentIdentifier, ComponentType};
use re_sdk_types::{ComponentDescriptor, ComponentSet};

use crate::typed_entity_collections::DatatypeMatch;

pub type DatatypeSet = std::collections::BTreeSet<arrow::datatypes::DataType>;

/// A visualizability specification that relies on a single required component with a known semantic type and a set of supported physical types.
///
/// A visualizer with this constraint is visualizable iff there's at least one component on the entity
/// that matches one of the given physical types.
///
/// We additionally store the `target_component` in order to know which component on the visualizer is the required one.
/// The `semantic_type` furthermore informs heuristics/recommendations for what Rerun type is a good fit.
///
/// If either side of the match is a known builtin enum, a semantic match is required
/// (plain physical type overlap like `UInt8` is not sufficient).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleRequiredComponentConstraint {
    /// The required component that this requirement is targeting.
    target_component: ComponentIdentifier,

    /// The semantic type the visualizer is working with.
    ///
    /// Matches with the semantic type are generally preferred.
    /// For builtin enum types, a semantic match is **required**.
    semantic_type: ComponentType,

    /// All supported physical Arrow data types.
    ///
    /// Has to contain the physical data type that is covered by the Rerun semantic type.
    physical_types: DatatypeSet,

    /// If false, ignores all static components.
    ///
    /// This is useful if you rely on ranges queries as done by the time series view.
    allow_static_data: bool,
}

impl From<SingleRequiredComponentConstraint> for VisualizabilityConstraints {
    fn from(req: SingleRequiredComponentConstraint) -> Self {
        Self::SingleRequiredComponent(req)
    }
}

impl SingleRequiredComponentConstraint {
    pub fn new<C: re_sdk_types::Component>(
        target_component_descriptor: &ComponentDescriptor,
    ) -> Self {
        re_log::debug_assert_eq!(
            target_component_descriptor.component_type,
            Some(C::name()),
            "Component type doesn't match target descriptor's type.",
        );

        Self {
            target_component: target_component_descriptor.component,
            semantic_type: C::name(),
            physical_types: std::iter::once(C::arrow_datatype()).collect(),
            allow_static_data: true,
        }
    }

    /// Adds additional physical types to the constraint.
    pub fn with_additional_physical_types(
        mut self,
        additional_types: impl IntoIterator<Item = arrow::datatypes::DataType>,
    ) -> Self {
        self.physical_types.extend(additional_types);
        self
    }

    /// Sets whether static-only components should be considered.
    pub fn with_allow_static_data(mut self, allow: bool) -> Self {
        self.allow_static_data = allow;
        self
    }

    /// The required component that this requirement is targeting.
    pub fn target_component(&self) -> ComponentIdentifier {
        self.target_component
    }

    /// Whether static-only components should be considered.
    ///
    /// If false, ignores all static components when evaluating this constraint.
    pub fn allow_static_data(&self) -> bool {
        self.allow_static_data
    }

    /// All supported physical Arrow data types.
    pub fn physical_types(&self) -> &DatatypeSet {
        &self.physical_types
    }

    /// Check if an incoming component's Arrow datatype matches this constraint.
    ///
    /// Returns `Some(DatatypeMatch)` when the component satisfies the constraint, `None` otherwise.
    ///
    /// If either side is a known builtin enum type, a semantic match is required.
    pub(crate) fn check_datatype_match(
        &self,
        known_enum_types: &nohash_hasher::IntSet<ComponentType>,
        incoming_arrow_datatype: &arrow::datatypes::DataType,
        incoming_component_type: Option<ComponentType>,
        incoming_component: ComponentIdentifier,
    ) -> Option<DatatypeMatch> {
        use re_arrow_combinators::extract_nested_fields;

        let is_physical_match = self.physical_types.contains(incoming_arrow_datatype);
        let is_semantic_match = incoming_component_type == Some(self.semantic_type);

        // Builtin enum types should only match via native semantics, never via physical datatype alone.
        // This applies in both directions:
        // - Incoming data that is an enum shouldn't match a non-enum visualizer physically
        //   (e.g. `FillMode` (UInt8) shouldn't be picked up by a visualizer that accepts UInt8).
        // - A visualizer that requires an enum type shouldn't accept non-enum data physically
        //   (e.g. a `FillMode` visualizer shouldn't pick up arbitrary UInt8 data).
        let incoming_is_enum =
            incoming_component_type.is_some_and(|ct| known_enum_types.contains(&ct));
        let constraint_is_enum = known_enum_types.contains(&self.semantic_type);
        if (incoming_is_enum || constraint_is_enum) && !is_semantic_match {
            return None;
        }

        match (is_physical_match, is_semantic_match) {
            (false, false) => {
                // No direct match - try nested field access
                extract_nested_fields(incoming_arrow_datatype, |dt| {
                    self.physical_types.contains(dt)
                })
                .map(|selectors| DatatypeMatch::PhysicalDatatypeOnly {
                    arrow_datatype: incoming_arrow_datatype.clone(),
                    component_type: incoming_component_type,
                    selectors: selectors.into(),
                })
            }

            (true, false) => Some(DatatypeMatch::PhysicalDatatypeOnly {
                arrow_datatype: incoming_arrow_datatype.clone(),
                component_type: incoming_component_type,
                selectors: Vec::new(),
            }),

            (true, true) => Some(DatatypeMatch::NativeSemantics {
                arrow_datatype: incoming_arrow_datatype.clone(),
                component_type: incoming_component_type,
            }),

            (false, true) => {
                re_log::warn_once!(
                    "Component {incoming_component:?} matched semantic type {:?} but none of the expected physical arrow types {incoming_arrow_datatype:?} for this semantic type.",
                    self.semantic_type,
                );
                None
            }
        }
    }
}

/// A visualizability constraint for image-like visualizers that require both a buffer and a format component.
///
/// - **Buffer**: always matched against `Blob`'s arrow datatype (see [`Self::buffer_arrow_datatype`]).
///   Semantic type is recorded as a hint for heuristics but is not required for matching.
///   I.e. it behaves exactly like the required component in [`SingleRequiredComponentConstraint`] (but with a fixed physical type).
/// - **Format**: matched by both arrow datatype AND semantic type (exact match required for both).
///
/// Buffer and format may arrive on an entity at different times (different chunks/events).
/// The subscriber tracks partial progress and promotes the entity to visualizable once both are satisfied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferAndFormatConstraint {
    /// The buffer component slot on the visualizer.
    buffer_target: ComponentIdentifier,

    /// The semantic type of the buffer component (hint only — not required for matching).
    buffer_semantic_type: ComponentType,

    /// The format component slot on the visualizer.
    format_target: ComponentIdentifier,

    /// The semantic type the format component must have (exact match required).
    format_semantic_type: ComponentType,

    /// The arrow datatype the format component must have (exact match required).
    format_arrow_datatype: arrow::datatypes::DataType,
}

impl BufferAndFormatConstraint {
    /// Creates a new buffer-and-format constraint.
    pub fn new<Buffer: re_sdk_types::Component, Format: re_sdk_types::Component>(
        buffer_descriptor: &ComponentDescriptor,
        format_descriptor: &ComponentDescriptor,
    ) -> Self {
        re_log::debug_assert_eq!(
            buffer_descriptor.component_type,
            Some(Buffer::name()),
            "Buffer component type doesn't match descriptor's type.",
        );
        re_log::debug_assert_eq!(
            format_descriptor.component_type,
            Some(Format::name()),
            "Format component type doesn't match descriptor's type.",
        );

        Self::new_with_type(
            buffer_descriptor.component,
            Buffer::name(),
            format_descriptor.component,
            Format::name(),
            Format::arrow_datatype(),
        )
    }

    /// Creates a new buffer-and-format constraint from raw identifiers and arrow datatypes.
    ///
    /// Unlike [`Self::new`], this does not require concrete component types and is useful for tests.
    pub fn new_with_type(
        buffer_target: ComponentIdentifier,
        buffer_semantic_type: ComponentType,
        format_target: ComponentIdentifier,
        format_semantic_type: ComponentType,
        format_arrow_datatype: arrow::datatypes::DataType,
    ) -> Self {
        Self {
            buffer_target,
            buffer_semantic_type,
            format_target,
            format_semantic_type,
            format_arrow_datatype,
        }
    }

    /// The arrow datatype used to match the buffer component.
    ///
    /// This is always [`re_sdk_types::datatypes::Blob`]'s arrow datatype, since all image-like
    /// buffers are opaque byte blobs regardless of the specific image archetype.
    // TODO(andreas): It would be great if we could support `BinaryArray` as well!
    pub fn buffer_arrow_datatype() -> arrow::datatypes::DataType {
        <re_sdk_types::datatypes::Blob as re_types_core::Loggable>::arrow_datatype()
    }

    /// The buffer component slot on the visualizer.
    pub fn buffer_target(&self) -> ComponentIdentifier {
        self.buffer_target
    }

    /// The format component slot on the visualizer.
    pub fn format_target(&self) -> ComponentIdentifier {
        self.format_target
    }

    /// Check if an incoming component matches the buffer side of this constraint.
    ///
    /// Matches by arrow datatype (direct or via nested field extraction).
    /// Semantic type match is recorded but not required.
    pub(crate) fn check_buffer_match(
        &self,
        incoming_arrow_datatype: &arrow::datatypes::DataType,
        descriptor: &re_sdk_types::ComponentDescriptor,
    ) -> Option<DatatypeMatch> {
        use re_arrow_combinators::extract_nested_fields;

        let is_physical_match = *incoming_arrow_datatype == Self::buffer_arrow_datatype();
        let is_semantic = descriptor.component_type == Some(self.buffer_semantic_type);

        match (is_physical_match, is_semantic) {
            (true, true) => Some(DatatypeMatch::NativeSemantics {
                arrow_datatype: incoming_arrow_datatype.clone(),
                component_type: descriptor.component_type,
            }),
            (true, false) => Some(DatatypeMatch::PhysicalDatatypeOnly {
                arrow_datatype: incoming_arrow_datatype.clone(),
                component_type: descriptor.component_type,
                selectors: Vec::new(),
            }),
            (false, _) => {
                // No direct match — try nested field access.
                extract_nested_fields(incoming_arrow_datatype, |dt| {
                    *dt == Self::buffer_arrow_datatype()
                })
                .map(|selectors| DatatypeMatch::PhysicalDatatypeOnly {
                    arrow_datatype: incoming_arrow_datatype.clone(),
                    component_type: descriptor.component_type,
                    selectors: selectors.into(),
                })
            }
        }
    }

    /// Check if an incoming component matches the format side of this constraint.
    ///
    /// Requires both arrow datatype AND semantic type to match.
    pub(crate) fn check_format_match(
        &self,
        incoming_arrow_datatype: &arrow::datatypes::DataType,
        descriptor: &re_sdk_types::ComponentDescriptor,
    ) -> bool {
        *incoming_arrow_datatype == self.format_arrow_datatype
            && descriptor.component_type == Some(self.format_semantic_type)
    }
}

impl From<BufferAndFormatConstraint> for VisualizabilityConstraints {
    fn from(req: BufferAndFormatConstraint) -> Self {
        Self::BufferAndFormat(req)
    }
}

/// Specifies how component requirements should be evaluated for visualizer entity matching.
/// Only on a successful match with an entity will the visualizer even be considered for that entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualizabilityConstraints {
    /// No component requirements - all entities are visualizable.
    None,

    /// Entity must have _any one_ of these components.
    AnyBuiltinComponent(ComponentSet),

    /// Entity must have _any one_ of these physical Arrow data types.
    ///
    /// For instance, we may not put views into the "recommended" section or visualizer entities proactively unless they support the native type.
    SingleRequiredComponent(SingleRequiredComponentConstraint),

    /// Entity must have both a buffer component (matched by arrow datatype) and a format
    /// component (matched by arrow datatype AND semantic type).
    ///
    /// See [`BufferAndFormatConstraint`] for details.
    BufferAndFormat(BufferAndFormatConstraint),
}
