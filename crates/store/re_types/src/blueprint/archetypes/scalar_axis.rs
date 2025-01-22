// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/scalar_axis.fbs".

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

/// **Archetype**: Configuration for the scalar axis of a plot.
#[derive(Clone, Debug, Default)]
pub struct ScalarAxis {
    /// The range of the axis.
    ///
    /// If unset, the range well be automatically determined based on the queried data.
    pub range: Option<SerializedComponentBatch>,

    /// If enabled, the Y axis range will remain locked to the specified range when zooming.
    pub zoom_lock: Option<SerializedComponentBatch>,
}

impl ScalarAxis {
    /// Returns the [`ComponentDescriptor`] for [`Self::range`].
    #[inline]
    pub fn descriptor_range() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ScalarAxis".into()),
            component_name: "rerun.components.Range1D".into(),
            archetype_field_name: Some("range".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::zoom_lock`].
    #[inline]
    pub fn descriptor_zoom_lock() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ScalarAxis".into()),
            component_name: "rerun.blueprint.components.LockRangeDuringZoom".into(),
            archetype_field_name: Some("zoom_lock".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.ScalarAxis".into()),
            component_name: "rerun.blueprint.components.ScalarAxisIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [ScalarAxis::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ScalarAxis::descriptor_range(),
            ScalarAxis::descriptor_zoom_lock(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            ScalarAxis::descriptor_indicator(),
            ScalarAxis::descriptor_range(),
            ScalarAxis::descriptor_zoom_lock(),
        ]
    });

impl ScalarAxis {
    /// The total number of components in the archetype: 0 required, 1 recommended, 2 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`ScalarAxis`] [`::re_types_core::Archetype`]
pub type ScalarAxisIndicator = ::re_types_core::GenericIndicatorComponent<ScalarAxis>;

impl ::re_types_core::Archetype for ScalarAxis {
    type Indicator = ScalarAxisIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.ScalarAxis".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Scalar axis"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: ScalarAxisIndicator = ScalarAxisIndicator::DEFAULT;
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
        let range = arrays_by_descr
            .get(&Self::descriptor_range())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_range()));
        let zoom_lock = arrays_by_descr
            .get(&Self::descriptor_zoom_lock())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_zoom_lock())
            });
        Ok(Self { range, zoom_lock })
    }
}

impl ::re_types_core::AsComponents for ScalarAxis {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Self::indicator().serialized(),
            self.range.clone(),
            self.zoom_lock.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for ScalarAxis {}

impl ScalarAxis {
    /// Create a new `ScalarAxis`.
    #[inline]
    pub fn new() -> Self {
        Self {
            range: None,
            zoom_lock: None,
        }
    }

    /// Update only some specific fields of a `ScalarAxis`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `ScalarAxis`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            range: Some(SerializedComponentBatch::new(
                crate::components::Range1D::arrow_empty(),
                Self::descriptor_range(),
            )),
            zoom_lock: Some(SerializedComponentBatch::new(
                crate::blueprint::components::LockRangeDuringZoom::arrow_empty(),
                Self::descriptor_zoom_lock(),
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
            self.range
                .map(|range| range.partitioned(_lengths.clone()))
                .transpose()?,
            self.zoom_lock
                .map(|zoom_lock| zoom_lock.partitioned(_lengths.clone()))
                .transpose()?,
        ];
        let indicator_column =
            ::re_types_core::indicator_column::<Self>(_lengths.into_iter().count())?;
        Ok(columns.into_iter().chain([indicator_column]).flatten())
    }

    /// Helper to partition the component data into unit-length sub-batches.
    ///
    /// This is semantically similar to calling [`Self::columns`] with `std::iter::take(1).repeat(n)`,
    /// where `n` is automatically guessed.
    #[inline]
    pub fn unary_columns(
        self,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>> {
        let len_range = self.range.as_ref().map(|b| b.array.len());
        let len_zoom_lock = self.zoom_lock.as_ref().map(|b| b.array.len());
        let len = None.or(len_range).or(len_zoom_lock).unwrap_or(0);
        self.columns(std::iter::repeat(1).take(len))
    }

    /// The range of the axis.
    ///
    /// If unset, the range well be automatically determined based on the queried data.
    #[inline]
    pub fn with_range(mut self, range: impl Into<crate::components::Range1D>) -> Self {
        self.range = try_serialize_field(Self::descriptor_range(), [range]);
        self
    }

    /// If enabled, the Y axis range will remain locked to the specified range when zooming.
    #[inline]
    pub fn with_zoom_lock(
        mut self,
        zoom_lock: impl Into<crate::blueprint::components::LockRangeDuringZoom>,
    ) -> Self {
        self.zoom_lock = try_serialize_field(Self::descriptor_zoom_lock(), [zoom_lock]);
        self
    }
}

impl ::re_byte_size::SizeBytes for ScalarAxis {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.range.heap_size_bytes() + self.zoom_lock.heap_size_bytes()
    }
}
