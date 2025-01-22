// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_collision_radius.fbs".

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

/// **Archetype**: Resolves collisions between the bounding circles, according to the radius of the nodes.
#[derive(Clone, Debug, Default)]
pub struct ForceCollisionRadius {
    /// Whether the collision force is enabled.
    ///
    /// The collision force resolves collisions between nodes based on the bounding circle defined by their radius.
    pub enabled: Option<SerializedComponentBatch>,

    /// The strength of the force.
    pub strength: Option<SerializedComponentBatch>,

    /// Specifies how often this force should be applied per iteration.
    ///
    /// Increasing this parameter can lead to better results at the cost of longer computation time.
    pub iterations: Option<SerializedComponentBatch>,
}

impl ForceCollisionRadius {
    /// Returns the [`ComponentDescriptor`] for [`Self::enabled`].
    #[inline]
    pub fn descriptor_enabled() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceCollisionRadius".into()),
            component_name: "rerun.blueprint.components.Enabled".into(),
            archetype_field_name: Some("enabled".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::strength`].
    #[inline]
    pub fn descriptor_strength() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceCollisionRadius".into()),
            component_name: "rerun.blueprint.components.ForceStrength".into(),
            archetype_field_name: Some("strength".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::iterations`].
    #[inline]
    pub fn descriptor_iterations() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceCollisionRadius".into()),
            component_name: "rerun.blueprint.components.ForceIterations".into(),
            archetype_field_name: Some("iterations".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceCollisionRadius".into()),
            component_name: "rerun.blueprint.components.ForceCollisionRadiusIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [ForceCollisionRadius::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ForceCollisionRadius::descriptor_enabled(),
            ForceCollisionRadius::descriptor_strength(),
            ForceCollisionRadius::descriptor_iterations(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ForceCollisionRadius::descriptor_indicator(),
            ForceCollisionRadius::descriptor_enabled(),
            ForceCollisionRadius::descriptor_strength(),
            ForceCollisionRadius::descriptor_iterations(),
        ]
    });

impl ForceCollisionRadius {
    /// The total number of components in the archetype: 0 required, 1 recommended, 3 optional
    pub const NUM_COMPONENTS: usize = 4usize;
}

/// Indicator component for the [`ForceCollisionRadius`] [`::re_types_core::Archetype`]
pub type ForceCollisionRadiusIndicator =
    ::re_types_core::GenericIndicatorComponent<ForceCollisionRadius>;

impl ::re_types_core::Archetype for ForceCollisionRadius {
    type Indicator = ForceCollisionRadiusIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.ForceCollisionRadius".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Force collision radius"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: ForceCollisionRadiusIndicator = ForceCollisionRadiusIndicator::DEFAULT;
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
        let strength = arrays_by_descr
            .get(&Self::descriptor_strength())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_strength()));
        let iterations = arrays_by_descr
            .get(&Self::descriptor_iterations())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_iterations())
            });
        Ok(Self {
            enabled,
            strength,
            iterations,
        })
    }
}

impl ::re_types_core::AsComponents for ForceCollisionRadius {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Self::indicator().serialized(),
            self.enabled.clone(),
            self.strength.clone(),
            self.iterations.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for ForceCollisionRadius {}

impl ForceCollisionRadius {
    /// Create a new `ForceCollisionRadius`.
    #[inline]
    pub fn new() -> Self {
        Self {
            enabled: None,
            strength: None,
            iterations: None,
        }
    }

    /// Update only some specific fields of a `ForceCollisionRadius`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `ForceCollisionRadius`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            enabled: Some(SerializedComponentBatch::new(
                crate::blueprint::components::Enabled::arrow_empty(),
                Self::descriptor_enabled(),
            )),
            strength: Some(SerializedComponentBatch::new(
                crate::blueprint::components::ForceStrength::arrow_empty(),
                Self::descriptor_strength(),
            )),
            iterations: Some(SerializedComponentBatch::new(
                crate::blueprint::components::ForceIterations::arrow_empty(),
                Self::descriptor_iterations(),
            )),
        }
    }

    /// Partitions the component data into multiple sub-batches.
    ///
    /// Specifically, this transforms the existing [`SerializedComponentBatch`]es data into [`SerializedComponentColumn`]s
    /// instead, via [`SerializedComponentBatch::partitioned`].
    ///
    /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
    ///
    /// The specified `lengths` must sum to the total length of the component batch.
    ///
    /// [`SerializedComponentColumn`]: [::re_types_core::SerializedComponentColumn]
    #[inline]
    pub fn columns<I>(
        self,
        _lengths: I,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>>
    where
        I: IntoIterator<Item = usize> + Clone,
    {
        let columns = [
            self.enabled
                .map(|enabled| enabled.partitioned(_lengths.clone()))
                .transpose()?,
            self.strength
                .map(|strength| strength.partitioned(_lengths.clone()))
                .transpose()?,
            self.iterations
                .map(|iterations| iterations.partitioned(_lengths.clone()))
                .transpose()?,
        ];
        let indicator_column =
            ::re_types_core::indicator_column::<Self>(_lengths.into_iter().count())?;
        Ok(columns.into_iter().chain([indicator_column]).flatten())
    }

    /// Whether the collision force is enabled.
    ///
    /// The collision force resolves collisions between nodes based on the bounding circle defined by their radius.
    #[inline]
    pub fn with_enabled(
        mut self,
        enabled: impl Into<crate::blueprint::components::Enabled>,
    ) -> Self {
        self.enabled = try_serialize_field(Self::descriptor_enabled(), [enabled]);
        self
    }

    /// The strength of the force.
    #[inline]
    pub fn with_strength(
        mut self,
        strength: impl Into<crate::blueprint::components::ForceStrength>,
    ) -> Self {
        self.strength = try_serialize_field(Self::descriptor_strength(), [strength]);
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

impl ::re_byte_size::SizeBytes for ForceCollisionRadius {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.enabled.heap_size_bytes()
            + self.strength.heap_size_bytes()
            + self.iterations.heap_size_bytes()
    }
}
