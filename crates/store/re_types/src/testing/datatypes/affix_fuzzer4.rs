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

use ::re_types_core::external::arrow;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

#[derive(Clone, Debug, PartialEq)]
pub enum AffixFuzzer4 {
    SingleRequired(crate::testing::datatypes::AffixFuzzer3),
    ManyRequired(Vec<crate::testing::datatypes::AffixFuzzer3>),
}

::re_types_core::macros::impl_into_cow!(AffixFuzzer4);

impl ::re_types_core::Loggable for AffixFuzzer4 {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::Union(
            UnionFields::new(
                vec![0, 1, 2],
                vec![
                    Field::new("_null_markers", DataType::Null, true),
                    Field::new(
                        "single_required",
                        <crate::testing::datatypes::AffixFuzzer3>::arrow_datatype(),
                        true,
                    ),
                    Field::new(
                        "many_required",
                        DataType::List(std::sync::Arc::new(Field::new(
                            "item",
                            <crate::testing::datatypes::AffixFuzzer3>::arrow_datatype(),
                            true,
                        ))),
                        false,
                    ),
                ],
            ),
            UnionMode::Dense,
        )
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
            // Dense Arrow union
            let data: Vec<_> = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    datum
                })
                .collect();
            let field_type_ids = [0, 1, 2];
            let fields = vec![
                Field::new("_null_markers", DataType::Null, true),
                Field::new(
                    "single_required",
                    <crate::testing::datatypes::AffixFuzzer3>::arrow_datatype(),
                    true,
                ),
                Field::new(
                    "many_required",
                    DataType::List(std::sync::Arc::new(Field::new(
                        "item",
                        <crate::testing::datatypes::AffixFuzzer3>::arrow_datatype(),
                        true,
                    ))),
                    false,
                ),
            ];
            let type_ids: Vec<i8> = data
                .iter()
                .map(|a| match a.as_deref() {
                    None => 0,
                    Some(Self::SingleRequired(_)) => 1i8,
                    Some(Self::ManyRequired(_)) => 2i8,
                })
                .collect();
            let offsets = {
                let mut single_required_offset = 0;
                let mut many_required_offset = 0;
                let mut nulls_offset = 0;
                data.iter()
                    .map(|v| match v.as_deref() {
                        None => {
                            let offset = nulls_offset;
                            nulls_offset += 1;
                            offset
                        }
                        Some(Self::SingleRequired(_)) => {
                            let offset = single_required_offset;
                            single_required_offset += 1;
                            offset
                        }
                        Some(Self::ManyRequired(_)) => {
                            let offset = many_required_offset;
                            many_required_offset += 1;
                            offset
                        }
                    })
                    .collect()
            };
            let children = vec![
                as_array_ref(NullArray::new(data.iter().filter(|v| v.is_none()).count())),
                {
                    let single_required: Vec<_> = data
                        .iter()
                        .filter_map(|datum| match datum.as_deref() {
                            Some(Self::SingleRequired(v)) => Some(v.clone()),
                            _ => None,
                        })
                        .collect();
                    let single_required_validity: Option<arrow::buffer::NullBuffer> = None;
                    {
                        _ = single_required_validity;
                        crate::testing::datatypes::AffixFuzzer3::to_arrow_opt(
                            single_required.into_iter().map(Some),
                        )?
                    }
                },
                {
                    let many_required: Vec<_> = data
                        .iter()
                        .filter_map(|datum| match datum.as_deref() {
                            Some(Self::ManyRequired(v)) => Some(v.clone()),
                            _ => None,
                        })
                        .collect();
                    let many_required_validity: Option<arrow::buffer::NullBuffer> = None;
                    {
                        let offsets = arrow::buffer::OffsetBuffer::<i32>::from_lengths(
                            many_required.iter().map(|datum| datum.len()),
                        );
                        let many_required_inner_data: Vec<_> =
                            many_required.into_iter().flatten().collect();
                        let many_required_inner_validity: Option<arrow::buffer::NullBuffer> = None;
                        as_array_ref(ListArray::try_new(
                            std::sync::Arc::new(Field::new(
                                "item",
                                <crate::testing::datatypes::AffixFuzzer3>::arrow_datatype(),
                                true,
                            )),
                            offsets,
                            {
                                _ = many_required_inner_validity;
                                crate::testing::datatypes::AffixFuzzer3::to_arrow_opt(
                                    many_required_inner_data.into_iter().map(Some),
                                )?
                            },
                            many_required_validity,
                        )?)
                    }
                },
            ];
            debug_assert_eq!(field_type_ids.len(), fields.len());
            debug_assert_eq!(fields.len(), children.len());
            as_array_ref(UnionArray::try_new(
                UnionFields::new(field_type_ids, fields),
                ScalarBuffer::from(type_ids),
                Some(offsets),
                children,
            )?)
        })
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{arrow_zip_validity::ZipValidity, Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow::array::UnionArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.testing.datatypes.AffixFuzzer4")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let arrow_data_type_ids = arrow_data.type_ids();
                let arrow_data_offsets = arrow_data
                    .offsets()
                    .ok_or_else(|| {
                        let expected = Self::arrow_datatype();
                        let actual = arrow_data.data_type().clone();
                        DeserializationError::datatype_mismatch(expected, actual)
                    })
                    .with_context("rerun.testing.datatypes.AffixFuzzer4")?;
                if arrow_data_type_ids.len() != arrow_data_offsets.len() {
                    return Err(DeserializationError::offset_slice_oob(
                        (0, arrow_data_type_ids.len()),
                        arrow_data_offsets.len(),
                    ))
                    .with_context("rerun.testing.datatypes.AffixFuzzer4");
                }
                let single_required = {
                    if arrow_data.type_ids().inner().len() <= 1 {
                        return Ok(Vec::new());
                    }
                    let arrow_data = arrow_data.child(1).as_ref();
                    crate::testing::datatypes::AffixFuzzer3::from_arrow_opt(arrow_data)
                        .with_context("rerun.testing.datatypes.AffixFuzzer4#single_required")?
                        .into_iter()
                        .collect::<Vec<_>>()
                };
                let many_required = {
                    if arrow_data.type_ids().inner().len() <= 2 {
                        return Ok(Vec::new());
                    }
                    let arrow_data = arrow_data.child(2).as_ref();
                    {
                        let arrow_data = arrow_data
                            .as_any()
                            .downcast_ref::<arrow::array::ListArray>()
                            .ok_or_else(|| {
                                let expected = DataType::List(std::sync::Arc::new(Field::new(
                                    "item",
                                    <crate::testing::datatypes::AffixFuzzer3>::arrow_datatype(),
                                    true,
                                )));
                                let actual = arrow_data.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context("rerun.testing.datatypes.AffixFuzzer4#many_required")?;
                        if arrow_data.is_empty() {
                            Vec::new()
                        } else {
                            let arrow_data_inner = {
                                let arrow_data_inner = &**arrow_data.values();
                                crate::testing::datatypes::AffixFuzzer3::from_arrow_opt(
                                    arrow_data_inner,
                                )
                                .with_context("rerun.testing.datatypes.AffixFuzzer4#many_required")?
                                .into_iter()
                                .collect::<Vec<_>>()
                            };
                            let offsets = arrow_data.offsets();
                            ZipValidity::new_with_validity(offsets.windows(2), arrow_data.nulls())
                                .map(|elem| {
                                    elem.map(|window| {
                                        let start = window[0] as usize;
                                        let end = window[1] as usize;
                                        if arrow_data_inner.len() < end {
                                            return Err(DeserializationError::offset_slice_oob(
                                                (start, end),
                                                arrow_data_inner.len(),
                                            ));
                                        }

                                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                        let data =
                                            unsafe { arrow_data_inner.get_unchecked(start..end) };
                                        let data = data
                                            .iter()
                                            .cloned()
                                            .map(Option::unwrap_or_default)
                                            .collect();
                                        Ok(data)
                                    })
                                    .transpose()
                                })
                                .collect::<DeserializationResult<Vec<Option<_>>>>()?
                        }
                        .into_iter()
                    }
                    .collect::<Vec<_>>()
                };
                arrow_data_type_ids
                    .iter()
                    .enumerate()
                    .map(|(i, typ)| {
                        let offset = arrow_data_offsets[i];
                        if *typ == 0 {
                            Ok(None)
                        } else {
                            Ok(Some(match typ {
                                1i8 => Self::SingleRequired({
                                    if offset as usize >= single_required.len() {
                                        return Err(DeserializationError::offset_oob(
                                            offset as _,
                                            single_required.len(),
                                        ))
                                        .with_context(
                                            "rerun.testing.datatypes.AffixFuzzer4#single_required",
                                        );
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { single_required.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(DeserializationError::missing_data)
                                        .with_context(
                                            "rerun.testing.datatypes.AffixFuzzer4#single_required",
                                        )?
                                }),
                                2i8 => Self::ManyRequired({
                                    if offset as usize >= many_required.len() {
                                        return Err(DeserializationError::offset_oob(
                                            offset as _,
                                            many_required.len(),
                                        ))
                                        .with_context(
                                            "rerun.testing.datatypes.AffixFuzzer4#many_required",
                                        );
                                    }

                                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                    unsafe { many_required.get_unchecked(offset as usize) }
                                        .clone()
                                        .ok_or_else(DeserializationError::missing_data)
                                        .with_context(
                                            "rerun.testing.datatypes.AffixFuzzer4#many_required",
                                        )?
                                }),
                                _ => {
                                    return Err(DeserializationError::missing_union_arm(
                                        Self::arrow_datatype(),
                                        "<invalid>",
                                        *typ as _,
                                    ));
                                }
                            }))
                        }
                    })
                    .collect::<DeserializationResult<Vec<_>>>()
                    .with_context("rerun.testing.datatypes.AffixFuzzer4")?
            }
        })
    }
}

impl ::re_byte_size::SizeBytes for AffixFuzzer4 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        #![allow(clippy::match_same_arms)]
        match self {
            Self::SingleRequired(v) => v.heap_size_bytes(),
            Self::ManyRequired(v) => v.heap_size_bytes(),
        }
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::testing::datatypes::AffixFuzzer3>::is_pod()
            && <Vec<crate::testing::datatypes::AffixFuzzer3>>::is_pod()
    }
}
