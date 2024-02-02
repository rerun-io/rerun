// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/series_line.fbs".

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

/// **Archetype**: Define the style properties for a line series in a chart.
#[derive(Clone, Debug)]
pub struct SeriesLine {
    /// Color for the corresponding series.
    pub color: Option<crate::components::Color>,

    /// Stroke width for the corresponding series.
    pub width: Option<crate::components::StrokeWidth>,
}

impl ::re_types_core::SizeBytes for SeriesLine {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.color.heap_size_bytes() + self.width.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::components::Color>>::is_pod()
            && <Option<crate::components::StrokeWidth>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.SeriesLineIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Color".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.StrokeWidth".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.SeriesLineIndicator".into(),
            "rerun.components.Color".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.StrokeWidth".into(),
        ]
    });

impl SeriesLine {
    pub const NUM_COMPONENTS: usize = 4usize;
}

/// Indicator component for the [`SeriesLine`] [`::re_types_core::Archetype`]
pub type SeriesLineIndicator = ::re_types_core::GenericIndicatorComponent<SeriesLine>;

impl ::re_types_core::Archetype for SeriesLine {
    type Indicator = SeriesLineIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.SeriesLine".into()
    }

    #[inline]
    fn indicator() -> MaybeOwnedComponentBatch<'static> {
        static INDICATOR: SeriesLineIndicator = SeriesLineIndicator::DEFAULT;
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
                .with_context("rerun.archetypes.SeriesLine#color")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let width = if let Some(array) = arrays_by_name.get("rerun.components.StrokeWidth") {
            <crate::components::StrokeWidth>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.SeriesLine#width")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self { color, width })
    }
}

impl ::re_types_core::AsComponents for SeriesLine {
    fn as_component_batches(&self) -> Vec<MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.color
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
            self.width
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
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

impl SeriesLine {
    pub fn new() -> Self {
        Self {
            color: None,
            width: None,
        }
    }

    #[inline]
    pub fn with_color(mut self, color: impl Into<crate::components::Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    #[inline]
    pub fn with_width(mut self, width: impl Into<crate::components::StrokeWidth>) -> Self {
        self.width = Some(width.into());
        self
    }
}
