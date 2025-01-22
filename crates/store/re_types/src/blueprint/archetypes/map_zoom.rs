// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/map_zoom.fbs".

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

/// **Archetype**: Configuration of the map view zoom level.
#[derive(Clone, Debug, Default)]
pub struct MapZoom {
    /// Zoom level for the map.
    ///
    /// Zoom level follow the [`OpenStreetMap` definition](https://wiki.openstreetmap.org/wiki/Zoom_levels).
    pub zoom: Option<SerializedComponentBatch>,
}

impl MapZoom {
    /// Returns the [`ComponentDescriptor`] for [`Self::zoom`].
    #[inline]
    pub fn descriptor_zoom() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.MapZoom".into()),
            component_name: "rerun.blueprint.components.ZoomLevel".into(),
            archetype_field_name: Some("zoom".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.blueprint.archetypes.MapZoom".into()),
            component_name: "rerun.blueprint.components.MapZoomIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [MapZoom::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [MapZoom::descriptor_zoom()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 2usize]> =
    once_cell::sync::Lazy::new(|| [MapZoom::descriptor_indicator(), MapZoom::descriptor_zoom()]);

impl MapZoom {
    /// The total number of components in the archetype: 0 required, 1 recommended, 1 optional
    pub const NUM_COMPONENTS: usize = 2usize;
}

/// Indicator component for the [`MapZoom`] [`::re_types_core::Archetype`]
pub type MapZoomIndicator = ::re_types_core::GenericIndicatorComponent<MapZoom>;

impl ::re_types_core::Archetype for MapZoom {
    type Indicator = MapZoomIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.blueprint.archetypes.MapZoom".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Map zoom"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: MapZoomIndicator = MapZoomIndicator::DEFAULT;
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
        let zoom = arrays_by_descr
            .get(&Self::descriptor_zoom())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_zoom()));
        Ok(Self { zoom })
    }
}

impl ::re_types_core::AsComponents for MapZoom {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [Self::indicator().serialized(), self.zoom.clone()]
            .into_iter()
            .flatten()
            .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for MapZoom {}

impl MapZoom {
    /// Create a new `MapZoom`.
    #[inline]
    pub fn new(zoom: impl Into<crate::blueprint::components::ZoomLevel>) -> Self {
        Self {
            zoom: try_serialize_field(Self::descriptor_zoom(), [zoom]),
        }
    }

    /// Update only some specific fields of a `MapZoom`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `MapZoom`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            zoom: Some(SerializedComponentBatch::new(
                crate::blueprint::components::ZoomLevel::arrow_empty(),
                Self::descriptor_zoom(),
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
        let columns = [self
            .zoom
            .map(|zoom| zoom.partitioned(_lengths.clone()))
            .transpose()?];
        let indicator_column =
            ::re_types_core::indicator_column::<Self>(_lengths.into_iter().count())?;
        Ok(columns.into_iter().chain([indicator_column]).flatten())
    }

    /// Zoom level for the map.
    ///
    /// Zoom level follow the [`OpenStreetMap` definition](https://wiki.openstreetmap.org/wiki/Zoom_levels).
    #[inline]
    pub fn with_zoom(mut self, zoom: impl Into<crate::blueprint::components::ZoomLevel>) -> Self {
        self.zoom = try_serialize_field(Self::descriptor_zoom(), [zoom]);
        self
    }
}

impl ::re_byte_size::SizeBytes for MapZoom {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.zoom.heap_size_bytes()
    }
}
