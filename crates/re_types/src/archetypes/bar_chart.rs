// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/bar_chart.fbs".

#![allow(trivial_numeric_casts)]
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

/// **Archetype**: A bar chart.
///
/// The x values will be the indices of the array, and the bar heights will be the provided values.
///
/// ## Example
///
/// ### Simple bar chart
/// ```ignore
/// //! Create and log a bar chart
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = rerun::RecordingStreamBuilder::new("rerun_example_bar_chart").memory()?;
///
///     rec.log(
///         "bar_chart",
///         &rerun::BarChart::new([8_i64, 4, 0, 9, 1, 4, 1, 6, 9, 0].as_slice()),
///     )?;
///
///     rerun::native_viewer::show(storage.take())?;
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
    /// The values. Should always be a rank-1 tensor.
    pub values: crate::components::TensorData,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.TensorData".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.BarChartIndicator".into()]);

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.InstanceKey".into()]);

static ALL_COMPONENTS: once_cell::sync::Lazy<[::re_types_core::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.TensorData".into(),
            "rerun.components.BarChartIndicator".into(),
            "rerun.components.InstanceKey".into(),
        ]
    });

impl BarChart {
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
    fn indicator() -> ::re_types_core::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: BarChartIndicator = BarChartIndicator::DEFAULT;
        ::re_types_core::MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [::re_types_core::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> ::re_types_core::DeserializationResult<Self> {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let values = {
            let array = arrays_by_name
                .get("rerun.components.TensorData")
                .ok_or_else(::re_types_core::DeserializationError::missing_data)
                .with_context("rerun.archetypes.BarChart#values")?;
            <crate::components::TensorData>::from_arrow_opt(&**array)
                .with_context("rerun.archetypes.BarChart#values")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(::re_types_core::DeserializationError::missing_data)
                .with_context("rerun.archetypes.BarChart#values")?
        };
        Ok(Self { values })
    }
}

impl ::re_types_core::AsComponents for BarChart {
    fn as_component_batches(&self) -> Vec<::re_types_core::MaybeOwnedComponentBatch<'_>> {
        re_tracing::profile_function!();
        use ::re_types_core::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.values as &dyn ::re_types_core::ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }
}

impl BarChart {
    pub fn new(values: impl Into<crate::components::TensorData>) -> Self {
        Self {
            values: values.into(),
        }
    }
}
