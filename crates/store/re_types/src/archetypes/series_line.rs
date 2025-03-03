// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/series_line.fbs".

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
use ::re_types_core::{ComponentBatch, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Archetype**: Define the style properties for a line series in a chart.
///
/// This archetype only provides styling information and should be logged as static
/// when possible. The underlying data needs to be logged to the same entity-path using
/// [`archetypes::Scalar`][crate::archetypes::Scalar].
///
/// ## Example
///
/// ### Line series
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_series_line_style").spawn()?;
///
///     // Set up plot styling:
///     // They are logged static as they don't change over time and apply to all timelines.
///     // Log two lines series under a shared root so that they show in the same plot by default.
///     rec.log_static(
///         "trig/sin",
///         &rerun::SeriesLine::new()
///             .with_color([255, 0, 0])
///             .with_name("sin(0.01t)")
///             .with_width(2.0),
///     )?;
///     rec.log_static(
///         "trig/cos",
///         &rerun::SeriesLine::new()
///             .with_color([0, 255, 0])
///             .with_name("cos(0.01t)")
///             .with_width(4.0),
///     )?;
///
///     for t in 0..((std::f32::consts::TAU * 2.0 * 100.0) as i64) {
///         rec.set_index("step", sequence=t);
///
///         // Log two time series under a shared root so that they show in the same plot by default.
///         rec.log("trig/sin", &rerun::Scalar::new((t as f64 / 100.0).sin()))?;
///         rec.log("trig/cos", &rerun::Scalar::new((t as f64 / 100.0).cos()))?;
///     }
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/1200w.png">
///   <img src="https://static.rerun.io/series_line_style/d2616d98b1e46bdb85849b8669154fdf058e3453/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, Default)]
pub struct SeriesLine {
    /// Color for the corresponding series.
    pub color: Option<SerializedComponentBatch>,

    /// Stroke width for the corresponding series.
    pub width: Option<SerializedComponentBatch>,

    /// Display name of the series.
    ///
    /// Used in the legend.
    pub name: Option<SerializedComponentBatch>,

    /// Which lines are visible.
    ///
    /// If not set, all line series on this entity are visible.
    /// Unlike with the regular visibility property of the entire entity, any series that is hidden
    /// via this property will still be visible in the legend.
    pub visible_series: Option<SerializedComponentBatch>,

    /// Configures the zoom-dependent scalar aggregation.
    ///
    /// This is done only if steps on the X axis go below a single pixel,
    /// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
    /// (and readability) in such situations as it prevents overdraw.
    pub aggregation_policy: Option<SerializedComponentBatch>,
}

impl SeriesLine {
    /// Returns the [`ComponentDescriptor`] for [`Self::color`].
    #[inline]
    pub fn descriptor_color() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.SeriesLine".into()),
            component_name: "rerun.components.Color".into(),
            archetype_field_name: Some("color".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::width`].
    #[inline]
    pub fn descriptor_width() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.SeriesLine".into()),
            component_name: "rerun.components.StrokeWidth".into(),
            archetype_field_name: Some("width".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::name`].
    #[inline]
    pub fn descriptor_name() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.SeriesLine".into()),
            component_name: "rerun.components.Name".into(),
            archetype_field_name: Some("name".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::visible_series`].
    #[inline]
    pub fn descriptor_visible_series() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.SeriesLine".into()),
            component_name: "rerun.components.SeriesVisible".into(),
            archetype_field_name: Some("visible_series".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::aggregation_policy`].
    #[inline]
    pub fn descriptor_aggregation_policy() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.SeriesLine".into()),
            component_name: "rerun.components.AggregationPolicy".into(),
            archetype_field_name: Some("aggregation_policy".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.SeriesLine".into()),
            component_name: "rerun.components.SeriesLineIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 0usize]> =
    once_cell::sync::Lazy::new(|| []);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [SeriesLine::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 5usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            SeriesLine::descriptor_color(),
            SeriesLine::descriptor_width(),
            SeriesLine::descriptor_name(),
            SeriesLine::descriptor_visible_series(),
            SeriesLine::descriptor_aggregation_policy(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 6usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            SeriesLine::descriptor_indicator(),
            SeriesLine::descriptor_color(),
            SeriesLine::descriptor_width(),
            SeriesLine::descriptor_name(),
            SeriesLine::descriptor_visible_series(),
            SeriesLine::descriptor_aggregation_policy(),
        ]
    });

impl SeriesLine {
    /// The total number of components in the archetype: 0 required, 1 recommended, 5 optional
    pub const NUM_COMPONENTS: usize = 6usize;
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
    fn display_name() -> &'static str {
        "Series line"
    }

    #[inline]
    fn indicator() -> SerializedComponentBatch {
        #[allow(clippy::unwrap_used)]
        SeriesLineIndicator::DEFAULT.serialized().unwrap()
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
        let color = arrays_by_descr
            .get(&Self::descriptor_color())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_color()));
        let width = arrays_by_descr
            .get(&Self::descriptor_width())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_width()));
        let name = arrays_by_descr
            .get(&Self::descriptor_name())
            .map(|array| SerializedComponentBatch::new(array.clone(), Self::descriptor_name()));
        let visible_series = arrays_by_descr
            .get(&Self::descriptor_visible_series())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_visible_series())
            });
        let aggregation_policy = arrays_by_descr
            .get(&Self::descriptor_aggregation_policy())
            .map(|array| {
                SerializedComponentBatch::new(array.clone(), Self::descriptor_aggregation_policy())
            });
        Ok(Self {
            color,
            width,
            name,
            visible_series,
            aggregation_policy,
        })
    }
}

impl ::re_types_core::AsComponents for SeriesLine {
    #[inline]
    fn as_serialized_batches(&self) -> Vec<SerializedComponentBatch> {
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            self.color.clone(),
            self.width.clone(),
            self.name.clone(),
            self.visible_series.clone(),
            self.aggregation_policy.clone(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for SeriesLine {}

impl SeriesLine {
    /// Create a new `SeriesLine`.
    #[inline]
    pub fn new() -> Self {
        Self {
            color: None,
            width: None,
            name: None,
            visible_series: None,
            aggregation_policy: None,
        }
    }

    /// Update only some specific fields of a `SeriesLine`.
    #[inline]
    pub fn update_fields() -> Self {
        Self::default()
    }

    /// Clear all the fields of a `SeriesLine`.
    #[inline]
    pub fn clear_fields() -> Self {
        use ::re_types_core::Loggable as _;
        Self {
            color: Some(SerializedComponentBatch::new(
                crate::components::Color::arrow_empty(),
                Self::descriptor_color(),
            )),
            width: Some(SerializedComponentBatch::new(
                crate::components::StrokeWidth::arrow_empty(),
                Self::descriptor_width(),
            )),
            name: Some(SerializedComponentBatch::new(
                crate::components::Name::arrow_empty(),
                Self::descriptor_name(),
            )),
            visible_series: Some(SerializedComponentBatch::new(
                crate::components::SeriesVisible::arrow_empty(),
                Self::descriptor_visible_series(),
            )),
            aggregation_policy: Some(SerializedComponentBatch::new(
                crate::components::AggregationPolicy::arrow_empty(),
                Self::descriptor_aggregation_policy(),
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
            self.color
                .map(|color| color.partitioned(_lengths.clone()))
                .transpose()?,
            self.width
                .map(|width| width.partitioned(_lengths.clone()))
                .transpose()?,
            self.name
                .map(|name| name.partitioned(_lengths.clone()))
                .transpose()?,
            self.visible_series
                .map(|visible_series| visible_series.partitioned(_lengths.clone()))
                .transpose()?,
            self.aggregation_policy
                .map(|aggregation_policy| aggregation_policy.partitioned(_lengths.clone()))
                .transpose()?,
        ];
        Ok(columns
            .into_iter()
            .flatten()
            .chain([::re_types_core::indicator_column::<Self>(
                _lengths.into_iter().count(),
            )?]))
    }

    /// Helper to partition the component data into unit-length sub-batches.
    ///
    /// This is semantically similar to calling [`Self::columns`] with `std::iter::take(1).repeat(n)`,
    /// where `n` is automatically guessed.
    #[inline]
    pub fn columns_of_unit_batches(
        self,
    ) -> SerializationResult<impl Iterator<Item = ::re_types_core::SerializedComponentColumn>> {
        let len_color = self.color.as_ref().map(|b| b.array.len());
        let len_width = self.width.as_ref().map(|b| b.array.len());
        let len_name = self.name.as_ref().map(|b| b.array.len());
        let len_visible_series = self.visible_series.as_ref().map(|b| b.array.len());
        let len_aggregation_policy = self.aggregation_policy.as_ref().map(|b| b.array.len());
        let len = None
            .or(len_color)
            .or(len_width)
            .or(len_name)
            .or(len_visible_series)
            .or(len_aggregation_policy)
            .unwrap_or(0);
        self.columns(std::iter::repeat(1).take(len))
    }

    /// Color for the corresponding series.
    #[inline]
    pub fn with_color(mut self, color: impl Into<crate::components::Color>) -> Self {
        self.color = try_serialize_field(Self::descriptor_color(), [color]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::Color`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_color`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_color(
        mut self,
        color: impl IntoIterator<Item = impl Into<crate::components::Color>>,
    ) -> Self {
        self.color = try_serialize_field(Self::descriptor_color(), color);
        self
    }

    /// Stroke width for the corresponding series.
    #[inline]
    pub fn with_width(mut self, width: impl Into<crate::components::StrokeWidth>) -> Self {
        self.width = try_serialize_field(Self::descriptor_width(), [width]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::StrokeWidth`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_width`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_width(
        mut self,
        width: impl IntoIterator<Item = impl Into<crate::components::StrokeWidth>>,
    ) -> Self {
        self.width = try_serialize_field(Self::descriptor_width(), width);
        self
    }

    /// Display name of the series.
    ///
    /// Used in the legend.
    #[inline]
    pub fn with_name(mut self, name: impl Into<crate::components::Name>) -> Self {
        self.name = try_serialize_field(Self::descriptor_name(), [name]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::Name`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_name`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_name(
        mut self,
        name: impl IntoIterator<Item = impl Into<crate::components::Name>>,
    ) -> Self {
        self.name = try_serialize_field(Self::descriptor_name(), name);
        self
    }

    /// Which lines are visible.
    ///
    /// If not set, all line series on this entity are visible.
    /// Unlike with the regular visibility property of the entire entity, any series that is hidden
    /// via this property will still be visible in the legend.
    #[inline]
    pub fn with_visible_series(
        mut self,
        visible_series: impl IntoIterator<Item = impl Into<crate::components::SeriesVisible>>,
    ) -> Self {
        self.visible_series =
            try_serialize_field(Self::descriptor_visible_series(), visible_series);
        self
    }

    /// Configures the zoom-dependent scalar aggregation.
    ///
    /// This is done only if steps on the X axis go below a single pixel,
    /// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
    /// (and readability) in such situations as it prevents overdraw.
    #[inline]
    pub fn with_aggregation_policy(
        mut self,
        aggregation_policy: impl Into<crate::components::AggregationPolicy>,
    ) -> Self {
        self.aggregation_policy =
            try_serialize_field(Self::descriptor_aggregation_policy(), [aggregation_policy]);
        self
    }

    /// This method makes it possible to pack multiple [`crate::components::AggregationPolicy`] in a single component batch.
    ///
    /// This only makes sense when used in conjunction with [`Self::columns`]. [`Self::with_aggregation_policy`] should
    /// be used when logging a single row's worth of data.
    #[inline]
    pub fn with_many_aggregation_policy(
        mut self,
        aggregation_policy: impl IntoIterator<Item = impl Into<crate::components::AggregationPolicy>>,
    ) -> Self {
        self.aggregation_policy =
            try_serialize_field(Self::descriptor_aggregation_policy(), aggregation_policy);
        self
    }
}

impl ::re_byte_size::SizeBytes for SeriesLine {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.color.heap_size_bytes()
            + self.width.heap_size_bytes()
            + self.name.heap_size_bytes()
            + self.visible_series.heap_size_bytes()
            + self.aggregation_policy.heap_size_bytes()
    }
}
