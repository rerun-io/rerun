// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/uvec4d.fbs".

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

/// **Datatype**: A uint vector in 4D space.
#[derive(Clone, Debug, Default, Copy, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct UVec4D(pub [u32; 4usize]);

::re_types_core::macros::impl_into_cow!(UVec4D);

impl ::re_types_core::Loggable for UVec4D {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow::datatypes::*;
        DataType::FixedSizeList(
            std::sync::Arc::new(Field::new("item", DataType::UInt32, false)),
            4,
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
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::{array::*, buffer::*, datatypes::*};

        #[allow(unused)]
        fn as_array_ref<T: Array + 'static>(t: T) -> ArrayRef {
            std::sync::Arc::new(t) as ArrayRef
        }
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
                let data0_inner_data: Vec<_> = data0
                    .into_iter()
                    .flat_map(|v| match v {
                        Some(v) => itertools::Either::Left(v.into_iter()),
                        None => itertools::Either::Right(
                            std::iter::repeat(Default::default()).take(4usize),
                        ),
                    })
                    .collect();
                let data0_inner_validity: Option<arrow::buffer::NullBuffer> =
                    data0_validity.as_ref().map(|validity| {
                        validity
                            .iter()
                            .map(|b| std::iter::repeat(b).take(4usize))
                            .flatten()
                            .collect::<Vec<_>>()
                            .into()
                    });
                as_array_ref(FixedSizeListArray::new(
                    std::sync::Arc::new(Field::new("item", DataType::UInt32, false)),
                    4,
                    as_array_ref(PrimitiveArray::<UInt32Type>::new(
                        ScalarBuffer::from(data0_inner_data.into_iter().collect::<Vec<_>>()),
                        data0_inner_validity,
                    )),
                    data0_validity,
                ))
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
                .downcast_ref::<arrow2::array::FixedSizeListArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.UVec4D#xyzw")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let offsets = (0..)
                    .step_by(4usize)
                    .zip((4usize..).step_by(4usize).take(arrow_data.len()));
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    arrow_data_inner
                        .as_any()
                        .downcast_ref::<UInt32Array>()
                        .ok_or_else(|| {
                            let expected = DataType::UInt32;
                            let actual = arrow_data_inner.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.UVec4D#xyzw")?
                        .into_iter()
                        .map(|opt| opt.copied())
                        .collect::<Vec<_>>()
                };
                arrow2::bitmap::utils::ZipValidity::new_with_validity(
                    offsets,
                    arrow_data.validity(),
                )
                .map(|elem| {
                    elem.map(|(start, end): (usize, usize)| {
                        debug_assert!(end - start == 4usize);
                        if end > arrow_data_inner.len() {
                            return Err(DeserializationError::offset_slice_oob(
                                (start, end),
                                arrow_data_inner.len(),
                            ));
                        }

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        let data = unsafe { arrow_data_inner.get_unchecked(start..end) };
                        let data = data.iter().cloned().map(Option::unwrap_or_default);

                        // NOTE: Unwrapping cannot fail: the length must be correct.
                        #[allow(clippy::unwrap_used)]
                        Ok(array_init::from_iter(data).unwrap())
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
        .with_context("rerun.datatypes.UVec4D#xyzw")
        .with_context("rerun.datatypes.UVec4D")?)
    }

    #[inline]
    fn from_arrow2(arrow_data: &dyn arrow2::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow::datatypes::*;
        use arrow2::{array::*, buffer::*};
        if let Some(validity) = arrow_data.validity() {
            if validity.unset_bits() != 0 {
                return Err(DeserializationError::missing_data());
            }
        }
        Ok({
            let slice = {
                let arrow_data = arrow_data
                    .as_any()
                    .downcast_ref::<arrow2::array::FixedSizeListArray>()
                    .ok_or_else(|| {
                        let expected = DataType::FixedSizeList(
                            std::sync::Arc::new(Field::new("item", DataType::UInt32, false)),
                            4,
                        );
                        let actual = arrow_data.data_type().clone();
                        DeserializationError::datatype_mismatch(expected, actual)
                    })
                    .with_context("rerun.datatypes.UVec4D#xyzw")?;
                let arrow_data_inner = &**arrow_data.values();
                bytemuck::cast_slice::<_, [_; 4usize]>(
                    arrow_data_inner
                        .as_any()
                        .downcast_ref::<UInt32Array>()
                        .ok_or_else(|| {
                            let expected = DataType::UInt32;
                            let actual = arrow_data_inner.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.UVec4D#xyzw")?
                        .values()
                        .as_slice(),
                )
            };
            {
                slice.iter().copied().map(Self).collect::<Vec<_>>()
            }
        })
    }
}

impl From<[u32; 4usize]> for UVec4D {
    #[inline]
    fn from(xyzw: [u32; 4usize]) -> Self {
        Self(xyzw)
    }
}

impl From<UVec4D> for [u32; 4usize] {
    #[inline]
    fn from(value: UVec4D) -> Self {
        value.0
    }
}

impl ::re_types_core::SizeBytes for UVec4D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <[u32; 4usize]>::is_pod()
    }
}
