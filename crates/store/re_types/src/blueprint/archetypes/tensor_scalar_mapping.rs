// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/tensor_scalar_mapping.fbs".

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

/// **Archetype**: Configures how tensor scalars are mapped to color.
#[derive(Clone, Debug, Default)]
pub struct TensorScalarMapping {
    /// Filter used when zooming in on the tensor.
    ///
    /// Note that the filter is applied to the scalar values *before* they are mapped to color.
    pub mag_filter: Option<SerializedComponentBatch>,

    /// How scalar values map to colors.
    pub colormap: Option<SerializedComponentBatch>,

    /// Gamma exponent applied to normalized values before mapping to color.
    ///
    /// Raises the normalized values to the power of this value before mapping to color.
    /// Acts like an inverse brightness. Defaults to 1.0.
    ///
    /// The final value for display is set as:
    /// `colormap( ((value - data_display_range.min) / (data_display_range.max - data_display_range.min)) ** gamma )`
    pub gamma: Option<SerializedComponentBatch>,
}

impl TensorScalarMapping {
    /// Returns the [`ComponentDescriptor`] for [`Self::mag_filter`].
    #[inline]
    pub fn descriptor_mag_filter() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.TensorScalarMapping".into()),
            component_name: "rerun.components.MagnificationFilter".into(),
            archetype_field_name: Some("mag_filter".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::colormap`].
    #[inline]
    pub fn descriptor_colormap() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.TensorScalarMapping".into()),
            component_name: "rerun.components.Colormap".into(),
            archetype_field_name: Some("colormap".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::gamma`].
    #[inline]
    pub fn descriptor_gamma() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.TensorScalarMapping".into()),
            component_name: "rerun.components.GammaCorrection".into(),
            archetype_field_name: Some("gamma".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.TensorScalarMapping".into()),
            component_name: "rerun.blueprint.components.TensorScalarMappingIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [TensorScalarMapping::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            TensorScalarMapping::descriptor_mag_filter(),
            TensorScalarMapping::descriptor_colormap(),
            TensorScalarMapping::descriptor_gamma(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            TensorScalarMapping::descriptor_indicator(),
            TensorScalarMapping::descriptor_mag_filter(),
            TensorScalarMapping::descriptor_colormap(),
            TensorScalarMapping::descriptor_gamma(),
        ]
    });

impl TensorScalarMapping {
    /// The total number of components in the archetype: 0 required, 1 recommended, 3 optional
    pub const NUM_COMPONENTS: usize = 4usize;
}

/// Indicator component for the [`TensorScalarMapping`] [`::re_types_core::Archetype`]
pub type TensorScalarMappingIndicator =
    ::re_types_core::GenericIndicatorComponent<TensorScalarMapping>;

impl ::re_types_core::Archetype for TensorScalarMapping {
    type Indicator = TensorScalarMappingIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.TensorScalarMapping".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Tensor scalar mapping"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: TensorScalarMappingIndicator = TensorScalarMappingIndicator::DEFAULT;
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
        let mag_filter = arrays_by_descr
            .get(&Self::descriptor_mag_filter())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_mag_filter())
            });
        let colormap = arrays_by_descr
            .get(&Self::descriptor_colormap())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_colormap()));
        let gamma = arrays_by_descr
            .get(&Self::descriptor_gamma())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_gamma()));
        Ok(Self {
            mag_filter,
            colormap,
            gamma,
        })
    }
}

impl ::re_types_core::AsComponents for TensorScalarMapping {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Self::indicator().serialized(),
            self.mag_filter.clone(),
            self.colormap.clone(),
            self.gamma.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for TensorScalarMapping {}

impl TensorScalarMapping {
    /// Create a new `TensorScalarMapping`.
    #[inline]
    pub fn new() -> Self {
        Self {
            mag_filter: None,
            colormap: None,
            gamma: None,
        }
    }

    /// Update only some specific fields of a `TensorScalarMapping`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `TensorScalarMapping`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            mag_filter: Some(SerializedComponentBatch::new(
                crate::components::MagnificationFilter::arrow_empty(),
                Self::descriptor_mag_filter(),
            )),
            colormap: Some(SerializedComponentBatch::new(
                crate::components::Colormap::arrow_empty(),
                Self::descriptor_colormap(),
            )),
            gamma: Some(SerializedComponentBatch::new(
                crate::components::GammaCorrection::arrow_empty(),
                Self::descriptor_gamma(),
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
            self.mag_filter
                .map(|mag_filter| mag_filter.partitioned(_lengths.clone()))
                .transpose()?,
            self.colormap
                .map(|colormap| colormap.partitioned(_lengths.clone()))
                .transpose()?,
            self.gamma
                .map(|gamma| gamma.partitioned(_lengths.clone()))
                .transpose()?,
        ];
        let indicator_column =
            ::re_types_core::indicator_column::<Self>(_lengths.into_iter().count())?;
        Ok(columns.into_iter().chain([indicator_column]).flatten())
    }

    /// Filter used when zooming in on the tensor.
    ///
    /// Note that the filter is applied to the scalar values *before* they are mapped to color.
    #[inline]
    pub fn with_mag_filter(
        mut self,
        mag_filter: impl Into<crate::components::MagnificationFilter>,
    ) -> Self {
        self.mag_filter = try_serialize_field(Self::descriptor_mag_filter(), [mag_filter]);
        self
    }

    /// How scalar values map to colors.
    #[inline]
    pub fn with_colormap(mut self, colormap: impl Into<crate::components::Colormap>) -> Self {
        self.colormap = try_serialize_field(Self::descriptor_colormap(), [colormap]);
        self
    }

    /// Gamma exponent applied to normalized values before mapping to color.
    ///
    /// Raises the normalized values to the power of this value before mapping to color.
    /// Acts like an inverse brightness. Defaults to 1.0.
    ///
    /// The final value for display is set as:
    /// `colormap( ((value - data_display_range.min) / (data_display_range.max - data_display_range.min)) ** gamma )`
    #[inline]
    pub fn with_gamma(mut self, gamma: impl Into<crate::components::GammaCorrection>) -> Self {
        self.gamma = try_serialize_field(Self::descriptor_gamma(), [gamma]);
        self
    }
}

impl ::re_byte_size::SizeBytes for TensorScalarMapping {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.mag_filter.heap_size_bytes()
            + self.colormap.heap_size_bytes()
            + self.gamma.heap_size_bytes()
    }
}
