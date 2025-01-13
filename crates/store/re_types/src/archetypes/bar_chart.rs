// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/bar_chart.fbs".

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

/// **Archetype**: A bar chart.
///
/// The x values will be the indices of the array, and the bar heights will be the provided values.
///
/// ## Example
///
/// ### Simple bar chart
/// ```ignore
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rec = rerun::RecordingStreamBuilder::new("rerun_example_bar_chart").spawn()?;
///
///     rec.log(
///         "bar_chart",
///         &rerun::BarChart::new([8_i64, 4, 0, 9, 1, 4, 1, 6, 9, 0].as_slice()),
///     )?;
///
///     Ok(())
/// }
/// ```
/// <center>
/// <picture>
///   <source media="(max-width: 480px)" srcset="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/480w.png">
///   <source media="(max-width: 768px)" srcset="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/768w.png">
///   <source media="(max-width: 1024px)" srcset="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/1024w.png">
///   <source media="(max-width: 1200px)" srcset="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/1200w.png">
///   <img src="https://static.rerun.io/barchart_simple/cf6014b18265edfcaa562c06526c0716b296b193/full.png" width="640">
/// </picture>
/// </center>
#[derive(Clone, Debug, PartialEq)]
pub struct BarChart {
    /// The values. Should always be a 1-dimensional tensor (i.e. a vector).
    pub values: crate::components::TensorData,

    /// The color of the bar chart
    pub color: Option<crate::components::Color>,
}

impl BarChart {
    /// Returns the [`ComponentDescriptor`] for [`Self::values`].
    #[inline]
    pub fn descriptor_values() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.BarChart".into()),
            component_name: "rerun.components.TensorData".into(),
            archetype_field_name: Some("values".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for [`Self::color`].
    #[inline]
    pub fn descriptor_color() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.BarChart".into()),
            component_name: "rerun.components.Color".into(),
            archetype_field_name: Some("color".into()),
        }
    }

    /// Returns the [`ComponentDescriptor`] for the associated indicator component.
    #[inline]
    pub fn descriptor_indicator() -> ComponentDescriptor {
        ComponentDescriptor {
            archetype_name: Some("rerun.archetypes.BarChart".into()),
            component_name: "rerun.components.BarChartIndicator".into(),
            archetype_field_name: None,
        }
    }
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [BarChart::descriptor_values()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [BarChart::descriptor_indicator()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 1usize]> =
    once_cell::sync::Lazy::new(|| [BarChart::descriptor_color()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[ComponentDescriptor; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            BarChart::descriptor_values(),
            BarChart::descriptor_indicator(),
            BarChart::descriptor_color(),
        ]
    });

impl BarChart {
    /// The total number of components in the archetype: 1 required, 1 recommended, 1 optional
    pub const NUM_COMPONENTS: usize = 3usize;
}

/// Indicator component for the [`BarChart`] [`::re_types_core::Archetype`]
pub type BarChartIndicator = ::re_types_core::GenericIndicatorComponent<BarChart>;

impl ::re_types_core::Archetype for BarChart {
    type Indicator = BarChartIndicator;

    #[inline]
    fn name() -> ::re_types_core::ArchetypeName {
        "rerun.archetypes.BarChart".into()
    }

    #[inline]
    fn display_name() -> &'static str {
        "Bar chart"
    }

    #[inline]
    fn indicator() -> ComponentBatchCowWithDescriptor<'static> {
        static INDICATOR: BarChartIndicator = BarChartIndicator::DEFAULT;
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
        let values = {
            let array = arrays_by_descr
                .get(&Self::descriptor_values())
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.BarChart#values")?;
            <crate::components::TensorData>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.BarChart#values")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(DeserializationError::missing_data)
                .with_context("rerun.archetypes.BarChart#values")?
        };
        let color = if let Some(array) = arrays_by_descr.get(&Self::descriptor_color()) {
            <crate::components::Color>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.BarChart#color")?
                .into_iter()
                .next()
                .flatten()
        } else {
            None
        };
        Ok(Self { values, color })
    }
}

impl ::re_types_core::AsComponents for BarChart {
    fn as_component_batches(&self) -> Vec<ComponentBatchCowWithDescriptor<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            (Some(&self.values as &dyn ComponentBatch)).map(|batch| {
                ::re_types_core::ComponentBatchCowWithDescriptor {
                    batch: batch.into(),
                    descriptor_override: Some(Self::descriptor_values()),
                }
            }),
            (self
                .color
                .as_ref()
                .map(|comp| (comp as &dyn ComponentBatch)))
            .map(|batch| ::re_types_core::ComponentBatchCowWithDescriptor {
                batch: batch.into(),
                descriptor_override: Some(Self::descriptor_color()),
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl ::re_types_core::ArchetypeReflectionMarker for BarChart {}

impl BarChart {
    /// Create a new `BarChart`.
    #[inline]
    pub fn new(values: impl Into<crate::components::TensorData>) -> Self {
        Self {
            values: values.into(),
            color: None,
        }
    }

    /// The color of the bar chart
    #[inline]
    pub fn with_color(mut self, color: impl Into<crate::components::Color>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl ::re_byte_size::SizeBytes for BarChart {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.values.heap_size_bytes() + self.color.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::components::TensorData>::is_pod() && <Option<crate::components::Color>>::is_pod()
    }
}
