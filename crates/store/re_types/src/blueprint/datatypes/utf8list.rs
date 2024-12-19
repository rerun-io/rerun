// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/utf8_list.fbs".

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

/// **Datatype**: A list of strings of text, encoded as UTF-8.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
#[repr(transparent)]
pub struct Utf8List(pub Vec<::re_types_core::ArrowString>);

::re_types_core::macros::impl_into_cow!(Utf8List);

impl ::re_types_core::Loggable for Utf8List {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::List(std::sync::Arc::new(Field::new(
            "item",
            DataType::Utf8,
            false,
        )))
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
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| datum.into_owned().0);
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_validity: Option<arrow::buffer::NullBuffer> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                let offsets = arrow::buffer::OffsetBuffer::<i32>::from_lengths(
                    data0
                        .iter()
                        .map(|opt| opt.as_ref().map_or(0, |datum| datum.len())),
                );
                let data0_inner_data: Vec<_> = data0.into_iter().flatten().flatten().collect();
                let data0_inner_validity: Option<arrow::buffer::NullBuffer> = None;
                as_array_ref(ListArray::try_new(
                    std::sync::Arc::new(Field::new("item", DataType::Utf8, false)),
                    offsets,
                    {
                        let offsets = arrow::buffer::OffsetBuffer::<i32>::from_lengths(
                            data0_inner_data.iter().map(|datum| datum.len()),
                        );
                        let inner_data: arrow::buffer::Buffer = data0_inner_data
                            .into_iter()
                            .flat_map(|s| s.into_arrow2_buffer())
                            .collect();

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        as_array_ref(unsafe {
                            StringArray::new_unchecked(offsets, inner_data, data0_inner_validity)
                        })
                    },
                    data0_validity,
                )?)
            }
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
                .downcast_ref::<arrow2::array::ListArray<i32>>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.blueprint.datatypes.Utf8List#value")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    {
                        let arrow_data_inner = arrow_data_inner
                            .as_any()
                            .downcast_ref::<arrow2::array::Utf8Array<i32>>()
                            .ok_or_else(|| {
                                let expected = DataType::Utf8;
                                let actual = arrow_data_inner.data_type().clone();
                                DeserializationError::datatype_mismatch(expected, actual)
                            })
                            .with_context("rerun.blueprint.datatypes.Utf8List#value")?;
                        let arrow_data_inner_buf = arrow_data_inner.values();
                        let offsets = arrow_data_inner.offsets();
                        arrow2::bitmap::utils::ZipValidity::new_with_validity(
                            offsets.windows(2),
                            arrow_data_inner.validity(),
                        )
                        .map(|elem| {
                            elem.map(|window| {
                                let start = window[0] as usize;
                                let end = window[1] as usize;
                                let len = end - start;
                                if arrow_data_inner_buf.len() < end {
                                    return Err(DeserializationError::offset_slice_oob(
                                        (start, end),
                                        arrow_data_inner_buf.len(),
                                    ));
                                }

                                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                                let data = unsafe {
                                    arrow_data_inner_buf.clone().sliced_unchecked(start, len)
                                };
                                Ok(data)
                            })
                            .transpose()
                        })
                        .map(|res_or_opt| {
                            res_or_opt.map(|res_or_opt| {
                                res_or_opt.map(|v| ::re_types_core::ArrowString::from(v))
                            })
                        })
                        .collect::<DeserializationResult<Vec<Option<_>>>>()
                        .with_context("rerun.blueprint.datatypes.Utf8List#value")?
                        .into_iter()
                    }
                    .collect::<Vec<_>>()
                };
                let offsets = arrow_data.offsets();
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    offsets.windows(2),
                    arrow_data.validity(),
                )
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
                        let data = unsafe { arrow_data_inner.get_unchecked(start..end) };
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
        .map(|v| v.ok_or_else(DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.blueprint.datatypes.Utf8List#value")
        .with_context("rerun.blueprint.datatypes.Utf8List")?)
    }
}

impl From<Vec<::re_types_core::ArrowString>> for Utf8List {
    #[inline]
    fn from(value: Vec<::re_types_core::ArrowString>) -> Self {
        Self(value)
    }
}

impl From<Utf8List> for Vec<::re_types_core::ArrowString> {
    #[inline]
    fn from(value: Utf8List) -> Self {
        value.0
    }
}

impl ::re_byte_size::SizeBytes for Utf8List {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <Vec<::re_types_core::ArrowString>>::is_pod()
    }
}
