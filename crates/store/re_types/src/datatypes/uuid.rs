// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/uuid.fbs".

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
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Datatype**: A 16-byte UUID.
#[derive(Clone, Debug, Default, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Uuid {
    /// The raw bytes representing the UUID.
    pub bytes: [u8; 16usize],
}

impl ::re_types_core::SizeBytes for Uuid {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.bytes.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <[u8; 16usize]>::is_pod()
    }
}

impl From<[u8; 16usize]> for Uuid {
    #[inline]
    fn from(bytes: [u8; 16usize]) -> Self {
        Self { bytes }
    }
}

impl From<Uuid> for [u8; 16usize] {
    #[inline]
    fn from(value: Uuid) -> Self {
        value.bytes
    }
}

::re_types_core::macros::impl_into_cow!(Uuid);

impl ::re_types_core::Loggable for Uuid {
    #[inline]
    fn arrow2_datatype() -> arrow2::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow2::datatypes::*;
        DataType::FixedSizeList(
            std::sync::Arc::new(Field::new("item", DataType::UInt8, false)),
            16usize,
        )
    }

    fn to_arrow2_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        #![allow(clippy::manual_is_variant_and)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, bytes): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum.map(|datum| datum.into_owned().bytes);
                    (datum.is_some(), datum)
                })
                .unzip();
            let bytes_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                let bytes_inner_data: Vec<_> = bytes
                    .into_iter()
                    .flat_map(|v| match v {
                        Some(v) => itertools::Either::Left(v.into_iter()),
                        None => itertools::Either::Right(
                            std::iter::repeat(Default::default()).take(16usize),
                        ),
                    })
                    .collect();
                let bytes_inner_bitmap: Option<arrow2::bitmap::Bitmap> =
                    bytes_bitmap.as_ref().map(|bitmap| {
                        bitmap
                            .iter()
                            .map(|b| std::iter::repeat(b).take(16usize))
                            .flatten()
                            .collect::<Vec<_>>()
                            .into()
                    });
                FixedSizeListArray::new(
                    Self::arrow2_datatype(),
                    PrimitiveArray::new(
                        DataType::UInt8,
                        bytes_inner_data.into_iter().collect(),
                        bytes_inner_bitmap,
                    )
                    .boxed(),
                    bytes_bitmap,
                )
                .boxed()
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
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::FixedSizeListArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow2_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.datatypes.Uuid#bytes")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let offsets = (0..)
                    .step_by(16usize)
                    .zip((16usize..).step_by(16usize).take(arrow_data.len()));
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    arrow_data_inner
                        .as_any()
                        .downcast_ref::<UInt8Array>()
                        .ok_or_else(|| {
                            let expected = DataType::UInt8;
                            let actual = arrow_data_inner.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.Uuid#bytes")?
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
                        debug_assert!(end - start == 16usize);
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
        .map(|res| res.map(|bytes| Some(Self { bytes })))
        .collect::<DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.datatypes.Uuid#bytes")
        .with_context("rerun.datatypes.Uuid")?)
    }

    #[inline]
    fn from_arrow2(arrow_data: &dyn arrow2::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
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
                            std::sync::Arc::new(Field::new("item", DataType::UInt8, false)),
                            16usize,
                        );
                        let actual = arrow_data.data_type().clone();
                        DeserializationError::datatype_mismatch(expected, actual)
                    })
                    .with_context("rerun.datatypes.Uuid#bytes")?;
                let arrow_data_inner = &**arrow_data.values();
                bytemuck::cast_slice::<_, [_; 16usize]>(
                    arrow_data_inner
                        .as_any()
                        .downcast_ref::<UInt8Array>()
                        .ok_or_else(|| {
                            let expected = DataType::UInt8;
                            let actual = arrow_data_inner.data_type().clone();
                            DeserializationError::datatype_mismatch(expected, actual)
                        })
                        .with_context("rerun.datatypes.Uuid#bytes")?
                        .values()
                        .as_slice(),
                )
            };
            {
                slice
                    .iter()
                    .copied()
                    .map(|bytes| Self { bytes })
                    .collect::<Vec<_>>()
            }
        })
    }
}
