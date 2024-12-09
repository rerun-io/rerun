// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

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
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffixFuzzer21 {
    pub single_half: half::f16,
    pub many_halves: ::re_types_core::ArrowBuffer<half::f16>,
}

::re_types_core::macros::impl_into_cow!(AffixFuzzer21);

impl ::re_types_core::Loggable for AffixFuzzer21 {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Struct(Fields::from(vec![
            Field::new("single_half", DataType::Float16, false),
            Field::new(
                "many_halves",
                DataType::List(std::sync::Arc::new(Field::new(
                    "item",
                    DataType::Float16,
                    false,
                ))),
                false,
            ),
        ]))
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        #![allow(clippy::manual_is_variant_and)]
        use ::re_types_core::{arrow_helpers::as_array_ref, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let fields = Fields::from(vec![
                Field::new("single_half", DataType::Float16, false),
                Field::new(
                    "many_halves",
                    DataType::List(std::sync::Arc::new(Field::new(
                        "item",
                        DataType::Float16,
                        false,
                    ))),
                    false,
                ),
            ]);
            let (somes, data): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    (datum.is_some(), datum)
                })
                .unzip();
            let validity: Option<arrow::buffer::NullBuffer> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            as_array_ref(StructArray::new(
                fields,
                vec![
                    {
                        let (somes, single_half): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.single_half.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let single_half_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        as_array_ref(PrimitiveArray::<Float16Type>::new(
                            ScalarBuffer::from(
                                single_half
                                    .into_iter()
                                    .map(|v| v.unwrap_or_default())
                                    .collect::<Vec<_>>(),
                            ),
                            single_half_validity,
                        ))
                    },
                    {
                        let (somes, many_halves): (Vec<_>, Vec<_>) = data
                            .iter()
                            .map(|datum| {
                                let datum = datum.as_ref().map(|datum| datum.many_halves.clone());
                                (datum.is_some(), datum)
                            })
                            .unzip();
                        let many_halves_validity: Option<arrow::buffer::NullBuffer> = {
                            let any_nones = somes.iter().any(|some| !*some);
                            any_nones.then(|| somes.into())
                        };
                        {
                            let offsets = arrow::buffer::OffsetBuffer::<i32>::from_lengths(
                                many_halves.iter().map(|opt| {
                                    opt.as_ref().map_or(0, |datum| datum.num_instances())
                                }),
                            );
                            let many_halves_inner_data: ScalarBuffer<_> = many_halves
                                .iter()
                                .flatten()
                                .map(|b| b.as_slice())
                                .collect::<Vec<_>>()
                                .concat()
                                .into();
                            let many_halves_inner_validity: Option<arrow::buffer::NullBuffer> =
                                None;
                            as_array_ref(ListArray::try_new(
                                std::sync::Arc::new(Field::new("item", DataType::Float16, false)),
                                offsets,
                                as_array_ref(PrimitiveArray::<Float16Type>::new(
                                    many_halves_inner_data,
                                    many_halves_inner_validity,
                                )),
                                many_halves_validity,
                            )?)
                        }
                    },
                ],
                validity,
            ))
        })
    }

    fn from_arrow2_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::datatypes::*;
        use arrow2::{array::*, buffer::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::StructArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.testing.datatypes.AffixFuzzer21")?;
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
                let single_half = {
                    if !arrays_by_name.contains_key("single_half") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "single_half",
                        ))
                        .with_context("rerun.testing.datatypes.AffixFuzzer21");
                    }
                    let arrow_data = &**arrays_by_name["single_half"];
                    arrow_data
                        .as_any()
                        .downcast_ref::<Float16Array>()
                        .ok_or_else(|| {
                            let expected = DataType::Float16;
                            let actual = arrow_data.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.testing.datatypes.AffixFuzzer21#single_half")?
                        .into_iter()
                        .map(|opt| opt.copied())
                };
                let many_halves = {
                    if !arrays_by_name.contains_key("many_halves") {
                        return Err(DeserializationError::missing_struct_field(
                            Self::arrow_datatype(),
                            "many_halves",
                        ))
                        .with_context("rerun.testing.datatypes.AffixFuzzer21");
                    }
                    let arrow_data = &**arrays_by_name["many_halves"];
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<arrow2::array::ListArray<i32>>()
                            .ok_or_else(|| {
                                let expected = DataType::List(std::sync::Arc::new(Field::new(
                                    "item",
                                    DataType::Float16,
                                    false,
                                )));
                                let actual = arrow_data.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context("rerun.testing.datatypes.AffixFuzzer21#many_halves")?;
                        if arrow_data.is_empty() {
                            Vec::new()
                        } else {
                            let arrow_data_inner = {
                                let arrow_data_inner = &**arrow_data.values();
                                arrow_data_inner
                                    .as_any()
                                    .downcast_ref::<Float16Array>()
                                    .ok_or_else(|| {
                                        let expected = DataType::Float16;
                                        let actual = arrow_data_inner.data_type().clone();
                                        DeserializationError::datatype_mismatch(expected, actual)
                                    })
                                    .with_context(
                                        "rerun.testing.datatypes.AffixFuzzer21#many_halves",
                                    )?
                                    .values()
                            };
                            let offsets = arrow_data.offsets();
                            arrow2::bitmap::utils::ZipValidity::new_with_validity(
                                offsets.iter().zip(offsets.lengths()),
                                arrow_data.validity(),
                            )
                            .map(|elem| {
                                elem.map(|(start, len)| {
                                    let start = *start as usize;
                                    let end = start + len;
                                    if end > arrow_data_inner.len() {
                                        return Err(DeserializationError::offset_slice_oob(
                                            (start, end),
                                            arrow_data_inner.len(),
                                        ));
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    let data = unsafe {
                                        arrow_data_inner
                                            .clone()
                                            .sliced_unchecked(start, end - start)
                                    };
                                    let data = ::re_types_core::ArrowBuffer::from(data);
                                    Ok(data)
                                })
                                .transpose()
                            })
                            .collect::<DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    ::itertools::izip!(single_half, many_halves),
                    arrow_data.validity(),
                )
                .map(|opt| {
                    opt.map(|(single_half, many_halves)| {
                        Ok(Self {
                            single_half: single_half
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context(
                                    "rerun.testing.datatypes.AffixFuzzer21#single_half",
                                )?,
                            many_halves: many_halves
                                .ok_or_else(DeserializationError::missing_data)
                                .with_context(
                                    "rerun.testing.datatypes.AffixFuzzer21#many_halves",
                                )?,
                        })
                    })
                    .transpose()
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.testing.datatypes.AffixFuzzer21")?
            }
        })
    }
}

impl ::re_types_core::SizeBytes for AffixFuzzer21 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.single_half.heap_size_bytes() + self.many_halves.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <half::f16>::is_pod() && <::re_types_core::ArrowBuffer<half::f16>>::is_pod()
    }
}
