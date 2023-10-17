// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

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

use ::re_types_core::external::arrow2;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffixFuzzer11(pub Option<::re_types_core::ArrowBuffer<f32>>);

impl From<Option<::re_types_core::ArrowBuffer<f32>>> for AffixFuzzer11 {
    #[inline]
    fn from(many_floats_optional: Option<::re_types_core::ArrowBuffer<f32>>) -> Self {
        Self(many_floats_optional)
    }
}

impl From<AffixFuzzer11> for Option<::re_types_core::ArrowBuffer<f32>> {
    #[inline]
    fn from(value: AffixFuzzer11) -> Self {
        value.0
    }
}

impl<'a> From<AffixFuzzer11> for ::std::borrow::Cow<'a, AffixFuzzer11> {
    #[inline]
    fn from(value: AffixFuzzer11) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a AffixFuzzer11> for ::std::borrow::Cow<'a, AffixFuzzer11> {
    #[inline]
    fn from(value: &'a AffixFuzzer11) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl ::re_types_core::Loggable for AffixFuzzer11 {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.testing.components.AffixFuzzer11".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::List(Box::new(Field {
            name: "item".to_owned(),
            data_type: DataType::Float32,
            is_nullable: false,
            metadata: [].into(),
        }))
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> ::re_types_core::SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            let (somes, data0): (Vec<_>, Vec<_>) = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    let datum = datum
                        .map(|datum| {
                            let Self(data0) = datum.into_owned();
                            data0
                        })
                        .flatten();
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                use ::re_types_core::external::arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                let data0_inner_data: Buffer<_> = data0
                    .iter()
                    .flatten()
                    .map(|b| b.as_slice())
                    .collect::<Vec<_>>()
                    .concat()
                    .into();
                let data0_inner_bitmap: Option<arrow2::bitmap::Bitmap> = None;
                let offsets =
                    arrow2::offset::Offsets::<i32>::try_from_lengths(data0.iter().map(|opt| {
                        opt.as_ref()
                            .map(|datum| datum.num_instances())
                            .unwrap_or_default()
                    }))
                    .unwrap()
                    .into();
                ListArray::new(
                    Self::arrow_datatype(),
                    offsets,
                    PrimitiveArray::new(DataType::Float32, data0_inner_data, data0_inner_bitmap)
                        .boxed(),
                    data0_bitmap,
                )
                .boxed()
            }
        })
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> ::re_types_core::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        re_tracing::profile_function!();
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<arrow2::array::ListArray<i32>>()
                .ok_or_else(|| {
                    ::re_types_core::DeserializationError::datatype_mismatch(
                        DataType::List(Box::new(Field {
                            name: "item".to_owned(),
                            data_type: DataType::Float32,
                            is_nullable: false,
                            metadata: [].into(),
                        })),
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.testing.components.AffixFuzzer11#many_floats_optional")?;
            if arrow_data.is_empty() {
                Vec::new()
            } else {
                let arrow_data_inner = {
                    let arrow_data_inner = &**arrow_data.values();
                    arrow_data_inner
                        .as_any()
                        .downcast_ref::<Float32Array>()
                        .ok_or_else(|| {
                            ::re_types_core::DeserializationError::datatype_mismatch(
                                DataType::Float32,
                                arrow_data_inner.data_type().clone(),
                            )
                        })
                        .with_context(
                            "rerun.testing.components.AffixFuzzer11#many_floats_optional",
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
                        if end as usize > arrow_data_inner.len() {
                            return Err(::re_types_core::DeserializationError::offset_slice_oob(
                                (start, end),
                                arrow_data_inner.len(),
                            ));
                        }

                        #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                        let data = unsafe {
                            arrow_data_inner
                                .clone()
                                .sliced_unchecked(start as usize, end - start as usize)
                        };
                        let data = ::re_types_core::ArrowBuffer::from(data);
                        Ok(data)
                    })
                    .transpose()
                })
                .collect::<::re_types_core::DeserializationResult<Vec<Option<_>>>>()?
            }
            .into_iter()
        }
        .map(Ok)
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<::re_types_core::DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.testing.components.AffixFuzzer11#many_floats_optional")
        .with_context("rerun.testing.components.AffixFuzzer11")?)
    }
}
