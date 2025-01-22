// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_many_body.fbs".

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

/// **Archetype**: A force between each pair of nodes that ressembles an electrical charge.
///
/// If `strength` is smaller than 0, it pushes nodes apart, if it is larger than 0 it pulls them together.
#[derive(Clone, Debug, Default)]
pub struct ForceManyBody {
    /// Whether the many body force is enabled.
    ///
    /// The many body force is applied on each pair of nodes in a way that ressembles an electrical charge. If the
    /// strength is smaller than 0, it pushes nodes apart; if it is larger than 0, it pulls them together.
    pub enabled: Option<SerializedComponentBatch>,

    /// The strength of the force.
    ///
    /// If `strength` is smaller than 0, it pushes nodes apart, if it is larger than 0 it pulls them together.
    pub strength: Option<SerializedComponentBatch>,
}

impl ForceManyBody {
    /// Returns the [`ComponentDescriptor`] for [`Self::enabled`].
    #[inline]
    pub fn descriptor_enabled() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceManyBody".into()),
            component_name: "rerun.blueprint.components.Enabled".into(),
            archetype_field_name: Some("enabled".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::strength`].
    #[inline]
    pub fn descriptor_strength() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceManyBody".into()),
            component_name: "rerun.blueprint.components.ForceStrength".into(),
            archetype_field_name: Some("strength".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ForceManyBody".into()),
            component_name: "rerun.blueprint.components.ForceManyBodyIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [ForceManyBody::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ForceManyBody::descriptor_enabled(),
            ForceManyBody::descriptor_strength(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ForceManyBody::descriptor_indicator(),
            ForceManyBody::descriptor_enabled(),
            ForceManyBody::descriptor_strength(),
        ]
    });

impl ForceManyBody {
    /// The total number of components in the archetype: 0 required, 1 recommended, 2 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`ForceManyBody`] [`::re_types_core::Archetype`]
pub type ForceManyBodyIndicator = ::re_types_core::GenericIndicatorComponent<ForceManyBody>;

impl ::re_types_core::Archetype for ForceManyBody {
    type Indicator = ForceManyBodyIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.ForceManyBody".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Force many body"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: ForceManyBodyIndicator = ForceManyBodyIndicator::DEFAULT;
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
        Ok(Self { enabled, strength })
    }
}

impl ::re_types_core::AsComponents for ForceManyBody {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Self::indicator().serialized(),
            self.enabled.clone(),
            self.strength.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for ForceManyBody {}

impl ForceManyBody {
    /// Create a new `ForceManyBody`.
    #[inline]
    pub fn new() -> Self {
        Self {
            enabled: None,
            strength: None,
        }
    }

    /// Update only some specific fields of a `ForceManyBody`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `ForceManyBody`.
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
        ];
        let indicator_column =
            ::re_types_core::indicator_column::<Self>(_lengths.into_iter().count())?;
        Ok(columns.into_iter().chain([indicator_column]).flatten())
    }

    /// Whether the many body force is enabled.
    ///
    /// The many body force is applied on each pair of nodes in a way that ressembles an electrical charge. If the
    /// strength is smaller than 0, it pushes nodes apart; if it is larger than 0, it pulls them together.
    #[inline]
    pub fn with_enabled(
        mut self,
        enabled: impl Into<crate::blueprint::components::Enabled>,
    ) -> Self {
        self.enabled = try_serialize_field(Self::descriptor_enabled(), [enabled]);
        self
    }

    /// The strength of the force.
    ///
    /// If `strength` is smaller than 0, it pushes nodes apart, if it is larger than 0 it pulls them together.
    #[inline]
    pub fn with_strength(
        mut self,
        strength: impl Into<crate::blueprint::components::ForceStrength>,
    ) -> Self {
        self.strength = try_serialize_field(Self::descriptor_strength(), [strength]);
        self
    }
}

impl ::re_byte_size::SizeBytes for ForceManyBody {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.enabled.heap_size_bytes() + self.strength.heap_size_bytes()
    }
}
