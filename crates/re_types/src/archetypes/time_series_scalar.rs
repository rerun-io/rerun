// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/archetypes/time_series_scalar.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// Log a double-precision scalar that will be visualized as a timeseries plot.
///
/// The current simulation time will be used for the time/X-axis, hence scalars
/// cannot be timeless!
///
/// ## Examples
///
/// ```ignore
/// //! Log a scalar over time.
///
/// use rerun::{archetypes::TimeSeriesScalar, RecordingStreamBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) = RecordingStreamBuilder::new("rerun_example_scalar").memory()?;
///
///     for step in 0..64 {
///         rec.set_time_sequence("step", step);
///         rec.log("scalar", &TimeSeriesScalar::new((step as f64 / 10.0).sin()))?;
///     }
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
///
/// ```ignore
/// //! Log a scalar over time.
///
/// use rerun::{archetypes::TimeSeriesScalar, RecordingStreamBuilder};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (rec, storage) =
///         RecordingStreamBuilder::new("rerun_example_scalar_multiple_plots").memory()?;
///     let mut lcg_state = 0_i64;
///
///     for t in 0..((std::f32::consts::TAU * 2.0 * 100.0) as i64) {
///         rec.set_time_sequence("step", t);
///
///         // Log two time series under a shared root so that they show in the same plot by default.
///         rec.log(
///             "trig/sin",
///             &TimeSeriesScalar::new((t as f64 / 100.0).sin())
///                 .with_label("sin(0.01t)")
///                 .with_color([255, 0, 0]),
///         )?;
///         rec.log(
///             "trig/cos",
///             &TimeSeriesScalar::new((t as f64 / 100.0).cos())
///                 .with_label("cos(0.01t)")
///                 .with_color([0, 255, 0]),
///         )?;
///
///         // Log scattered points under a different root so that it shows in a different plot by default.
///         lcg_state = (1140671485_i64
///             .wrapping_mul(lcg_state)
///             .wrapping_add(128201163))
///             % 16777216; // simple linear congruency generator
///         rec.log(
///             "scatter/lcg",
///             &TimeSeriesScalar::new(lcg_state as f64).with_scattered(true),
///         )?;
///     }
///
///     rerun::native_viewer::show(storage.take())?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct TimeSeriesScalar {
    /// The scalar value to log.
    pub scalar: crate::components::Scalar,

    /// An optional radius for the point.
    ///
    /// Points within a single line do not have to share the same radius, the line
    /// will have differently sized segments as appropriate.
    ///
    /// If all points within a single entity path (i.e. a line) share the same
    /// radius, then this radius will be used as the line width too. Otherwise, the
    /// line will use the default width of `1.0`.
    pub radius: Option<crate::components::Radius>,

    /// Optional color for the scalar entry.
    ///
    /// If left unspecified, a pseudo-random color will be used instead. That
    /// same color will apply to all points residing in the same entity path
    /// that don't have a color specified.
    ///
    /// Points within a single line do not have to share the same color, the line
    /// will have differently colored segments as appropriate.
    /// If all points within a single entity path (i.e. a line) share the same
    /// color, then this color will be used as the line color in the plot legend.
    /// Otherwise, the line will appear gray in the legend.
    pub color: Option<crate::components::Color>,

    /// An optional label for the point.
    ///
    /// TODO(#1289): This won't show up on points at the moment, as our plots don't yet
    /// support displaying labels for individual points.
    /// If all points within a single entity path (i.e. a line) share the same label, then
    /// this label will be used as the label for the line itself. Otherwise, the
    /// line will be named after the entity path. The plot itself is named after
    /// the space it's in.
    pub label: Option<crate::components::Text>,

    /// Specifies whether a point in a scatter plot should form a continuous line.
    ///
    /// If set to true, this scalar will be drawn as a point, akin to a scatterplot.
    /// Otherwise, it will form a continuous line with its neighbors.
    /// Points within a single line do not have to all share the same scatteredness:
    /// the line will switch between a scattered and a continuous representation as
    /// required.
    pub scattered: Option<crate::components::ScalarScattering>,
}

static REQUIRED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 1usize]> =
    once_cell::sync::Lazy::new(|| ["rerun.components.Scalar".into()]);

static RECOMMENDED_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Color".into(),
            "rerun.components.Radius".into(),
            "rerun.components.TimeSeriesScalarIndicator".into(),
        ]
    });

static OPTIONAL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 3usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.InstanceKey".into(),
            "rerun.components.ScalarScattering".into(),
            "rerun.components.Text".into(),
        ]
    });

static ALL_COMPONENTS: once_cell::sync::Lazy<[crate::ComponentName; 7usize]> =
    once_cell::sync::Lazy::new(|| {
        [
            "rerun.components.Scalar".into(),
            "rerun.components.Color".into(),
            "rerun.components.Radius".into(),
            "rerun.components.TimeSeriesScalarIndicator".into(),
            "rerun.components.InstanceKey".into(),
            "rerun.components.ScalarScattering".into(),
            "rerun.components.Text".into(),
        ]
    });

impl TimeSeriesScalar {
    pub const NUM_COMPONENTS: usize = 7usize;
}

/// Indicator component for the [`TimeSeriesScalar`] [`crate::Archetype`]
pub type TimeSeriesScalarIndicator = crate::GenericIndicatorComponent<TimeSeriesScalar>;

impl crate::Archetype for TimeSeriesScalar {
    type Indicator = TimeSeriesScalarIndicator;

    #[inline]
    fn name() -> crate::ArchetypeName {
        "rerun.archetypes.TimeSeriesScalar".into()
    }

    #[inline]
    fn indicator() -> crate::MaybeOwnedComponentBatch<'static> {
        static INDICATOR: TimeSeriesScalarIndicator = TimeSeriesScalarIndicator::DEFAULT;
        crate::MaybeOwnedComponentBatch::Ref(&INDICATOR)
    }

    #[inline]
    fn required_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        REQUIRED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn recommended_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        RECOMMENDED_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn optional_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        OPTIONAL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn all_components() -> ::std::borrow::Cow<'static, [crate::ComponentName]> {
        ALL_COMPONENTS.as_slice().into()
    }

    #[inline]
    fn try_from_arrow(
        arrow_data: impl IntoIterator<
            Item = (::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>),
        >,
    ) -> crate::DeserializationResult<Self> {
        use crate::{Loggable as _, ResultExt as _};
        let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data
            .into_iter()
            .map(|(field, array)| (field.name, array))
            .collect();
        let scalar = {
            let array = arrays_by_name
                .get("scalar")
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.TimeSeriesScalar#scalar")?;
            <crate::components::Scalar>::try_from_arrow_opt(&**array)
                .with_context("rerun.archetypes.TimeSeriesScalar#scalar")?
                .into_iter()
                .next()
                .flatten()
                .ok_or_else(crate::DeserializationError::missing_data)
                .with_context("rerun.archetypes.TimeSeriesScalar#scalar")?
        };
        let radius = if let Some(array) = arrays_by_name.get("radius") {
            Some({
                <crate::components::Radius>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.TimeSeriesScalar#radius")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.TimeSeriesScalar#radius")?
            })
        } else {
            None
        };
        let color = if let Some(array) = arrays_by_name.get("color") {
            Some({
                <crate::components::Color>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.TimeSeriesScalar#color")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.TimeSeriesScalar#color")?
            })
        } else {
            None
        };
        let label = if let Some(array) = arrays_by_name.get("label") {
            Some({
                <crate::components::Text>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.TimeSeriesScalar#label")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.TimeSeriesScalar#label")?
            })
        } else {
            None
        };
        let scattered = if let Some(array) = arrays_by_name.get("scattered") {
            Some({
                <crate::components::ScalarScattering>::try_from_arrow_opt(&**array)
                    .with_context("rerun.archetypes.TimeSeriesScalar#scattered")?
                    .into_iter()
                    .next()
                    .flatten()
                    .ok_or_else(crate::DeserializationError::missing_data)
                    .with_context("rerun.archetypes.TimeSeriesScalar#scattered")?
            })
        } else {
            None
        };
        Ok(Self {
            scalar,
            radius,
            color,
            label,
            scattered,
        })
    }
}

impl crate::AsComponents for TimeSeriesScalar {
    fn as_component_batches(&self) -> Vec<crate::MaybeOwnedComponentBatch<'_>> {
        use crate::Archetype as _;
        [
            Some(Self::indicator()),
            Some((&self.scalar as &dyn crate::ComponentBatch).into()),
            self.radius
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
            self.color
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
            self.label
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
            self.scattered
                .as_ref()
                .map(|comp| (comp as &dyn crate::ComponentBatch).into()),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    #[inline]
    fn num_instances(&self) -> usize {
        1
    }

    #[inline]
    fn try_to_arrow(
        &self,
    ) -> crate::SerializationResult<
        Vec<(::arrow2::datatypes::Field, Box<dyn ::arrow2::array::Array>)>,
    > {
        use crate::{Loggable as _, ResultExt as _};
        Ok([
            {
                Some({
                    let array = <crate::components::Scalar>::try_to_arrow([&self.scalar]);
                    array.map(|array| {
                        let datatype = ::arrow2::datatypes::DataType::Extension(
                            "rerun.components.Scalar".into(),
                            Box::new(array.data_type().clone()),
                            None,
                        );
                        (
                            ::arrow2::datatypes::Field::new("scalar", datatype, false),
                            array,
                        )
                    })
                })
                .transpose()
                .with_context("rerun.archetypes.TimeSeriesScalar#scalar")?
            },
            {
                self.radius
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::Radius>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.Radius".into(),
                                Box::new(array.data_type().clone()),
                                None,
                            );
                            (
                                ::arrow2::datatypes::Field::new("radius", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.TimeSeriesScalar#radius")?
            },
            {
                self.color
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::Color>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.Color".into(),
                                Box::new(array.data_type().clone()),
                                None,
                            );
                            (
                                ::arrow2::datatypes::Field::new("color", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.TimeSeriesScalar#color")?
            },
            {
                self.label
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::Text>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.Text".into(),
                                Box::new(array.data_type().clone()),
                                None,
                            );
                            (
                                ::arrow2::datatypes::Field::new("label", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.TimeSeriesScalar#label")?
            },
            {
                self.scattered
                    .as_ref()
                    .map(|single| {
                        let array = <crate::components::ScalarScattering>::try_to_arrow([single]);
                        array.map(|array| {
                            let datatype = ::arrow2::datatypes::DataType::Extension(
                                "rerun.components.ScalarScattering".into(),
                                Box::new(array.data_type().clone()),
                                None,
                            );
                            (
                                ::arrow2::datatypes::Field::new("scattered", datatype, false),
                                array,
                            )
                        })
                    })
                    .transpose()
                    .with_context("rerun.archetypes.TimeSeriesScalar#scattered")?
            },
        ]
        .into_iter()
        .flatten()
        .collect())
    }
}

impl TimeSeriesScalar {
    pub fn new(scalar: impl Into<crate::components::Scalar>) -> Self {
        Self {
            scalar: scalar.into(),
            radius: None,
            color: None,
            label: None,
            scattered: None,
        }
    }

    pub fn with_radius(mut self, radius: impl Into<crate::components::Radius>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    pub fn with_color(mut self, color: impl Into<crate::components::Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<crate::components::Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_scattered(
        mut self,
        scattered: impl Into<crate::components::ScalarScattering>,
    ) -> Self {
        self.scattered = Some(scattered.into());
        self
    }
}
