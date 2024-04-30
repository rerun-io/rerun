// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/datatypes/visible_time_range.fbs".

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

use crate::external::arrow2;
use crate::ComponentName;
use crate::SerializationResult;
use crate::{ComponentBatch, MaybeOwnedComponentBatch};
use crate::{DeserializationError, DeserializationResult};

/// **Datatype**: Type of boundary for visible history.
#[derive(Clone, Debug, Copy)]
pub struct VisibleTimeRangeBoundary {
    /// Type of the boundary.
    pub kind: crate::datatypes::VisibleTimeRangeBoundaryKind,

    /// Value of the boundary (ignored for `Infinite` type).
    pub time: crate::datatypes::TimeInt,
}

impl crate::SizeBytes for VisibleTimeRangeBoundary {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.kind.heap_size_bytes() + self.time.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::VisibleTimeRangeBoundaryKind>::is_pod()
            && <crate::datatypes::TimeInt>::is_pod()
    }
}

crate::macros::impl_into_cow!(VisibleTimeRangeBoundary);

impl crate::Loggable for VisibleTimeRangeBoundary {
    type Name = crate::DatatypeName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.datatypes.VisibleTimeRangeBoundary".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Struct(std::sync::Arc::new(vec![
            Field::new(
                "kind",
                <crate::datatypes::VisibleTimeRangeBoundaryKind>::arrow_datatype(),
                false,
            ),
            Field::new("time", <crate::datatypes::TimeInt>::arrow_datatype(), false),
        ]))
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            StructArray::new(
                <crate::datatypes::VisibleTimeRangeBoundary>::arrow_datatype(),
                vec![
                    {
                        let (somes, kind): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| {
                                    let Self { kind, .. } = &**datum;
                                    kind.clone()
                                });
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let kind_bitmap: Option<arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            _ = kind_bitmap;
                            crate::datatypes::VisibleTimeRangeBoundaryKind::to_arrow_opt(kind)?
                        }
                    },
                    {
                        let (somes, time): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| {
                                    let Self { time, .. } = &**datum;
                                    time.clone()
                                });
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let time_bitmap: Option<arrow2::bitmap::Bitmap> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        PrimitiveArray::new(
                            DataType::Int64,
                            time.into_iter()
                                .map(|datum| {
                                    datum
                                        .map(|datum| {
                                            let crate::datatypes::TimeInt(data0) = datum;
                                            data0
                                        })
                                        .unwrap_or_default()
                                })
                                .collect(),
                            time_bitmap,
                        )
                        .boxed()
                    },
                ],
                bitmap,
            )
            .boxed()
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use crate::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.VisibleTimeRangeBoundary")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let (arrow_data_fields, arrow_data_arrays) =
                    (arrow_data.fields(), arrow_data.values());
                let arrays_by_name: ::std::collections::HashMap<_, _> = arrow_data_fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .zip(arrow_data_arrays)
                    .collect();
                let kind = {
                    if !arrays_by_name.contains_key("kind") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "kind",
                        ))
                        .with_context("rerun.datatypes.VisibleTimeRangeBoundary");
                    }
                    let arrow_data = &**arrays_by_name["kind"];
                    crate::datatypes::VisibleTimeRangeBoundaryKind::from_arrow_opt(arrow_data)
                        .with_context("rerun.datatypes.VisibleTimeRangeBoundary#kind")?
                        .into_iter()
                };
                let time = {
                    if !arrays_by_name.contains_key("time") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "time",
                        ))
                        .with_context("rerun.datatypes.VisibleTimeRangeBoundary");
                    }
                    let arrow_data = &**arrays_by_name["time"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<Int64Array>()
                        .ok_or_else(|| {
                            let expected = DataType::Int64;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.VisibleTimeRangeBoundary#time")?
                        .into_iter()
                        .map(|opt| opt.copied())
                        .map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::TimeInt(v)))
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(kind, time),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(kind, time)| {
                        Ok(Self {
                            kind: kind
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.datatypes.VisibleTimeRangeBoundary#kind")?,
                            time: time
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context("rerun.datatypes.VisibleTimeRangeBoundary#time")?,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.datatypes.VisibleTimeRangeBoundary")?
            }
        })
    }
}
