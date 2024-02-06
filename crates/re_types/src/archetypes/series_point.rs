// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/series_point.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Define the style properties for a point series in a chart.
#[derive(Clone, Debug)]
pub struct SeriesPoint {
    /// Color for the corresponding series.
    pub color: Option<crate::components::Color>,

    /// What shape to use to represent the point
    pub marker: Option<crate::components::MarkerShape>,

    /// Display name of the series.
    ///
    /// Used in the legend.
    pub name: Option<crate::components::Name>,

    /// Size of the markers.
    pub size: Option<Vec<crate::components::MarkerSize>>,
}

impl ::re_types_core::SizeBytes for SeriesPoint {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.color.heap_size_bytes()
            + self.marker.heap_size_bytes()
            + self.name.heap_size_bytes()
            + self.size.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::components::Color>>::is_pod()
            && <Option<crate::components::MarkerShape>>::is_pod()
            && <Option<crate::components::Name>>::is_pod()
            && <Option<Vec<crate::components::MarkerSize>>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.SeriesPointIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Color".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.MarkerShape".into(),
            "rerun.components.MarkerSize".into(),
            "rerun.components.Name".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 6usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.SeriesPointIndicator".into(),
            "rerun.components.Color".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.MarkerShape".into(),
            "rerun.components.MarkerSize".into(),
            "rerun.components.Name".into(),
        ]
    });

impl SeriesPoint {
    pub const NUM_COMPONENTS: usize = 6usize;
}

/// Indicator component for the [`SeriesPoint`] [`::re_types_core::Archetype`]
pub type SeriesPointIndicator = ::re_types_core::GenericIndicatorComponent<SeriesPoint>;

impl ::re_types_core::Archetype for SeriesPoint {
    type Indicator = SeriesPointIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.SeriesPoint".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: SeriesPointIndicator = SeriesPointIndicator::DEFAULT;
        MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let color = if let Some(array) = arrays_by_name.get("rerun.components.Color") {
            <crate::components::Color>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SeriesPoint#color")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let marker = if let Some(array) = arrays_by_name.get("rerun.components.MarkerShape") {
            <crate::components::MarkerShape>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SeriesPoint#marker")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let name = if let Some(array) = arrays_by_name.get("rerun.components.Name") {
            <crate::components::Name>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SeriesPoint#name")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let size = if let Some(array) = arrays_by_name.get("rerun.components.MarkerSize") {
            Some({
                <crate::components::MarkerSize>::from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.SeriesPoint#size")?
                    .into_iter()
                    .map(|v| v.ok_or_else(DeserializationError::missing_data))
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.archetypes.SeriesPoint#size")?
            })
        } else {
            None
        };
        Ok(Self {
            color,
            marker,
            name,
            size,
        })
    }
}

impl ::re_types_core::AsComponents for SeriesPoint {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.color
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.marker
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.name
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.size
                .as_ref()
                .map(|comp_batch| (comp_batch as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        0
    }
}

impl SeriesPoint {
    pub fn new() -> Self {
        Self {
            color: None,
            marker: None,
            name: None,
            size: None,
        }
    }

    #[inline]
    pub fn with_color(mut self, color: impl Into<crate::components::Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    #[inline]
    pub fn with_marker(mut self, marker: impl Into<crate::components::MarkerShape>) -> Self {
        self.marker = Some(marker.into());
        self
    }

    #[inline]
    pub fn with_name(mut self, name: impl Into<crate::components::Name>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[inline]
    pub fn with_size(
        mut self,
        size: impl IntoIterator<Item = impl Into<crate::components::MarkerSize>>,
    ) -> Self {
        self.size = Some(size.into_iter().map(Into::into).collect());
        self
    }
}
