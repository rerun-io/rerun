// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_link.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Aims to achieve a target distance between two nodes that are connected by an edge.
#[derive(Clone, Debug, Default)]
pub struct ForceLink {
    /// Whether the link force is enabled.
    ///
    /// The link force aims to achieve a target distance between two nodes that are connected by one ore more edges.
    pub enabled: Option<SerializedComponentBatch>,

    /// The target distance between two nodes.
    pub distance: Option<SerializedComponentBatch>,

    /// Specifies how often this force should be applied per iteration.
    ///
    /// Increasing this parameter can lead to better results at the cost of longer computation time.
    pub iterations: Option<SerializedComponentBatch>,
}

impl ForceLink {
    /// Returns the [`ComponentDescriptor`] for [`Self::enabled`].
    #[inline]
    pub fn descriptor_enabled() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceLink".into()),
            component_name: "rerun.blueprint.components.Enabled".into(),
            archetype_field_name: Some("enabled".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::distance`].
    #[inline]
    pub fn descriptor_distance() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceLink".into()),
            component_name: "rerun.blueprint.components.ForceDistance".into(),
            archetype_field_name: Some("distance".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::iterations`].
    #[inline]
    pub fn descriptor_iterations() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceLink".into()),
            component_name: "rerun.blueprint.components.ForceIterations".into(),
            archetype_field_name: Some("iterations".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceLink".into()),
            component_name: "rerun.blueprint.components.ForceLinkIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [ForceLink::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ForceLink::descriptor_enabled(),
            ForceLink::descriptor_distance(),
            ForceLink::descriptor_iterations(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ForceLink::descriptor_indicator(),
            ForceLink::descriptor_enabled(),
            ForceLink::descriptor_distance(),
            ForceLink::descriptor_iterations(),
        ]
    });

impl ForceLink {
    /// The total number of components in the archetype: 0 required, 1 recommended, 3 optional
    pub const NUM_COMPONENTS: usize = 4usize;
}

/// Indicator component for the [`ForceLink`] [`::re_types_core::Archetype`]
pub type ForceLinkIndicator = ::re_types_core::GenericIndicatorComponent<ForceLink>;

impl ::re_types_core::Archetype for ForceLink {
    type Indicator = ForceLinkIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.ForceLink".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Force link"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: ForceLinkIndicator = ForceLinkIndicator::DEFAULT;
        ComponentBatchCowWithDescriptor::new(&INDICATOR as &dyn ::re_types_core::ComponentBatch)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentDescriptor]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentDescriptor, arrow::array::ArrayRef)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_descr: ::nohash_hasher::IntMap<_, _> = arrow_data.into_iter().collect();
        let enabled = arrays_by_descr
            .get(&Self::descriptor_enabled())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_enabled()));
        let distance = arrays_by_descr
            .get(&Self::descriptor_distance())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_distance()));
        let iterations = arrays_by_descr
            .get(&Self::descriptor_iterations())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_iterations())
            });
        Ok(Self {
            enabled,
            distance,
            iterations,
        })
    }
}

impl ::re_types_core::AsComponents for ForceLink {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Self::indicator().serialized(),
            self.enabled.clone(),
            self.distance.clone(),
            self.iterations.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for ForceLink {}

impl ForceLink {
    /// Create a new `ForceLink`.
    #[inline]
    pub fn new() -> Self {
        Self {
            enabled: None,
            distance: None,
            iterations: None,
        }
    }

    /// Update only some specific fields of a `ForceLink`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `ForceLink`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            enabled: Some(SerializedComponentBatch::new(
                crate::blueprint::components::Enabled::arrow_empty(),
                Self::descriptor_enabled(),
            )),
            distance: Some(SerializedComponentBatch::new(
                crate::blueprint::components::ForceDistance::arrow_empty(),
                Self::descriptor_distance(),
            )),
            iterations: Some(SerializedComponentBatch::new(
                crate::blueprint::components::ForceIterations::arrow_empty(),
                Self::descriptor_iterations(),
            )),
        }
    }

    /// Whether the link force is enabled.
    ///
    /// The link force aims to achieve a target distance between two nodes that are connected by one ore more edges.
    #[inline]
    pub fn with_enabled(
        mut self,
        enabled: impl Into<crate::blueprint::components::Enabled>,
    ) -> Self {
        self.enabled = try_serialize_field(Self::descriptor_enabled(), [enabled]);
        self
    }

    /// The target distance between two nodes.
    #[inline]
    pub fn with_distance(
        mut self,
        distance: impl Into<crate::blueprint::components::ForceDistance>,
    ) -> Self {
        self.distance = try_serialize_field(Self::descriptor_distance(), [distance]);
        self
    }

    /// Specifies how often this force should be applied per iteration.
    ///
    /// Increasing this parameter can lead to better results at the cost of longer computation time.
    #[inline]
    pub fn with_iterations(
        mut self,
        iterations: impl Into<crate::blueprint::components::ForceIterations>,
    ) -> Self {
        self.iterations = try_serialize_field(Self::descriptor_iterations(), [iterations]);
        self
    }
}

impl ::re_byte_size::SizeBytes for ForceLink {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.enabled.heap_size_bytes()
            + self.distance.heap_size_bytes()
            + self.iterations.heap_size_bytes()
    }
}
