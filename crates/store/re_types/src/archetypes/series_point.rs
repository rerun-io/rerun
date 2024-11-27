// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/series_point.fbs".

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

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Define the style properties for a point series in a chart.
///
/// This archetype only provides styling information and should be logged as static
/// when possible. The underlying data needs to be logged to the same entity-path using
/// [`archetypes::Scalar`][crate::archetypes::Scalar].
///
/// ## Example
///
/// ### Point series
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_series_point_style").spawn()?;
///
///     // Set up plot styling:
///     // They are logged static as they don't change over time and apply to all timelines.
///     // Log two point series under a shared root so that they show in the same plot by default.
///     rec.log_static(
///         "trig/sin",
///         &rerun::SeriesPoint::new()
///             .with_color([255, 0, 0])
///             .with_name("sin(0.01t)")
///             .with_marker(rerun::components::MarkerShape::Circle)
///             .with_marker_size(4.0),
///     )?;
///     rec.log_static(
///         "trig/cos",
///         &rerun::SeriesPoint::new()
///             .with_color([0, 255, 0])
///             .with_name("cos(0.01t)")
///             .with_marker(rerun::components::MarkerShape::Cross)
///             .with_marker_size(2.0),
///     )?;
///
///     for t in 0..((std::f32::consts::TAU * 2.0 * 10.0) as i64) {
///         rec.set_time_sequence("step", t);
///
///         // Log two time series under a shared root so that they show in the same plot by default.
///         rec.log("trig/sin", &rerun::Scalar::new((t as f64 / 10.0).sin()))?;
///         rec.log("trig/cos", &rerun::Scalar::new((t as f64 / 10.0).cos()))?;
///     }
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/1200w.png">
///   <img src="https://static.rerun.io/series_point_style/82207a705da6c086b28ce161db1db9e8b12258b7/full.png" width="640">
/// </picture>
/// </center>
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

    /// Size of the marker.
    pub marker_size: Option<crate::components::MarkerSize>,
}

impl ::re_types_core::SizeBytes for SeriesPoint {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.color.heap_size_bytes()
            + self.marker.heap_size_bytes()
            + self.name.heap_size_bytes()
            + self.marker_size.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Option<crate::components::Color>>::is_pod()
            && <Option<crate::components::MarkerShape>>::is_pod()
            && <Option<crate::components::Name>>::is_pod()
            && <Option<crate::components::MarkerSize>>::is_pod()
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.SeriesPointIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 4usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Color".into(),
            "rerun.components.MarkerShape".into(),
            "rerun.components.Name".into(),
            "rerun.components.MarkerSize".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentName; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.SeriesPointIndicator".into(),
            "rerun.components.Color".into(),
            "rerun.components.MarkerShape".into(),
            "rerun.components.Name".into(),
            "rerun.components.MarkerSize".into(),
        ]
    });

impl SeriesPoint {
    /// The total number of components in the archetype: 0 required, 1 recommended, 4 optional
    pub const NUM_COMPONENTS: usize = 5usize;
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
    fn display_name() -> &'static str {
        "Series point"
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
    fn from_arrow2_components(
        arrow_data: impl IntoIterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>,
    ) -> DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(name, array)| (name.full_name(), array))
            .collect();
        let color = if let Some(array) = arrays_by_name.get("rerun.components.Color") {
            <crate::components::Color>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.SeriesPoint#color")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let marker = if let Some(array) = arrays_by_name.get("rerun.components.MarkerShape") {
            <crate::components::MarkerShape>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.SeriesPoint#marker")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let name = if let Some(array) = arrays_by_name.get("rerun.components.Name") {
            <crate::components::Name>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.SeriesPoint#name")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        let marker_size = if let Some(array) = arrays_by_name.get("rerun.components.MarkerSize") {
            <crate::components::MarkerSize>::from_arrow2_opt(&**array)
                .with_context("rerun.archetypes.SeriesPoint#marker_size")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self {
            color,
            marker,
            name,
            marker_size,
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
            self.marker_size
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for SeriesPoint {}

impl SeriesPoint {
    /// Create a new `SeriesPoint`.
    #[inline]
    pub fn new() -> Self {
        Self {
            color: None,
            marker: None,
            name: None,
            marker_size: None,
        }
    }

    /// Color for the corresponding series.
    #[inline]
    pub fn with_color(mut self, color: impl Into<crate::components::Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// What shape to use to represent the point
    #[inline]
    pub fn with_marker(mut self, marker: impl Into<crate::components::MarkerShape>) -> Self {
        self.marker = Some(marker.into());
        self
    }

    /// Display name of the series.
    ///
    /// Used in the legend.
    #[inline]
    pub fn with_name(mut self, name: impl Into<crate::components::Name>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Size of the marker.
    #[inline]
    pub fn with_marker_size(
        mut self,
        marker_size: impl Into<crate::components::MarkerSize>,
    ) -> Self {
        self.marker_size = Some(marker_size.into());
        self
    }
}
