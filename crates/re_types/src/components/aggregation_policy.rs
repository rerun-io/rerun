// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/aggregation_policy.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
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

/// **Component**: Policy for aggregation of multiple scalar plot values.
///
/// This is used for lines in plots when the X axis distance of individual points goes below a single pixel,
/// i.e. a single pixel covers more than one tick worth of data. It can greatly improve performance
/// (and readability) in such situations as it prevents overdraw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AggregationPolicy {
    /// No aggregation.
    Off = 1,

    /// Average all points in the range together.
    Average = 2,

    /// Keep only the maximum values in the range.
    Max = 3,

    /// Keep only the minimum values in the range.
    Min = 4,

    /// Keep both the minimum and maximum values in the range.
    ///
    /// This will yield two aggregated points instead of one, effectively creating a vertical line.
    #[default]
    MinMax = 5,

    /// Find both the minimum and maximum values in the range, then use the average of those.
    MinMaxAverage = 6,
}

impl AggregationPolicy {
    /// All the different enum variants.
    pub const ALL: [Self; 6] = [
        Self::Off,
        Self::Average,
        Self::Max,
        Self::Min,
        Self::MinMax,
        Self::MinMaxAverage,
    ];
}

impl ::re_types_core::SizeBytes for AggregationPolicy {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl std::fmt::Display for AggregationPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::Average => write!(f, "Average"),
            Self::Max => write!(f, "Max"),
            Self::Min => write!(f, "Min"),
            Self::MinMax => write!(f, "MinMax"),
            Self::MinMaxAverage => write!(f, "MinMaxAverage"),
        }
    }
}

::re_types_core::macros::impl_into_cow!(AggregationPolicy);

impl ::re_types_core::Loggable for AggregationPolicy {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.AggregationPolicy".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Union(
            std::sync::Arc::new(vec![
                Field::new("_null_markers", DataType::Null, true),
                Field::new("Off", DataType::Null, true),
                Field::new("Average", DataType::Null, true),
                Field::new("Max", DataType::Null, true),
                Field::new("Min", DataType::Null, true),
                Field::new("MinMax", DataType::Null, true),
                Field::new("MinMaxAverage", DataType::Null, true),
            ]),
            Some(std::sync::Arc::new(vec![
                0i32, 1i32, 2i32, 3i32, 4i32, 5i32, 6i32,
            ])),
            UnionMode::Sparse,
        )
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            // Sparse Arrow union
            let data: Vec<_> = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    datum
                })
                .collect();
            let num_variants = 6usize;
            let types = data
                .iter()
                .map(|a| match a.as_deref() {
                    None => 0,
                    Some(value) => *value as i8,
                })
                .collect();
            let fields: Vec<_> =
                std::iter::repeat(NullArray::new(DataType::Null, data.len()).boxed())
                    .take(1 + num_variants)
                    .collect();
            UnionArray::new(Self::arrow_datatype(), types, fields, None).boxed()
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::UnionArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.components.AggregationPolicy")?;
            let arrow_data_types = arrow_data.types();
            arrow_data_types
                .iter()
                .map(|typ| match typ {
                    0 => Ok(None),
                    1 => Ok(Some(Self::Off)),
                    2 => Ok(Some(Self::Average)),
                    3 => Ok(Some(Self::Max)),
                    4 => Ok(Some(Self::Min)),
                    5 => Ok(Some(Self::MinMax)),
                    6 => Ok(Some(Self::MinMaxAverage)),
                    _ => Err(DeserializationError::missing_union_arm(
                        Self::arrow_datatype(),
                        "<invalid>",
                        *typ as _,
                    )),
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.components.AggregationPolicy")?
        })
    }
}
