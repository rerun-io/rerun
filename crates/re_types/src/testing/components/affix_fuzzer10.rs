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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AffixFuzzer10(pub Option<::re_types_core::ArrowString>);

impl From<Option<::re_types_core::ArrowString>> for AffixFuzzer10 {
    #[inline]
    fn from(single_string_optional: Option<::re_types_core::ArrowString>) -> Self {
        Self(single_string_optional)
    }
}

impl From<AffixFuzzer10> for Option<::re_types_core::ArrowString> {
    #[inline]
    fn from(value: AffixFuzzer10) -> Self {
        value.0
    }
}

impl<'a> From<AffixFuzzer10> for ::std::borrow::Cow<'a, AffixFuzzer10> {
    #[inline]
    fn from(value: AffixFuzzer10) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a AffixFuzzer10> for ::std::borrow::Cow<'a, AffixFuzzer10> {
    #[inline]
    fn from(value: &'a AffixFuzzer10) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl ::re_types_core::Loggable for AffixFuzzer10 {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.testing.components.AffixFuzzer10".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::Utf8
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
            let data0_bitmap: Option<::re_types_core::external::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                let inner_data: ::re_types_core::external::arrow2::buffer::Buffer<u8> =
                    data0.iter().flatten().flat_map(|s| s.0.clone()).collect();
                let offsets =
                    ::re_types_core::external::arrow2::offset::Offsets::<i32>::try_from_lengths(
                        data0
                            .iter()
                            .map(|opt| opt.as_ref().map(|datum| datum.0.len()).unwrap_or_default()),
                    )
                    .unwrap()
                    .into();

                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                unsafe {
                    Utf8Array::<i32>::new_unchecked(
                        Self::arrow_datatype(),
                        offsets,
                        inner_data,
                        data0_bitmap,
                    )
                }
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
                .downcast_ref::<::re_types_core::external::arrow2::array::Utf8Array<i32>>()
                .ok_or_else(|| {
                    ::re_types_core::DeserializationError::datatype_mismatch(
                        DataType::Utf8,
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.testing.components.AffixFuzzer10#single_string_optional")?;
            let arrow_data_buf = arrow_data.values();
            let offsets = arrow_data.offsets();
            ::re_types_core::external::arrow2::bitmap::utils::ZipValidity::new_with_validity(
                offsets.iter().zip(offsets.lengths()),
                arrow_data.validity(),
            )
            .map(|elem| {
                elem.map(|(start, len)| {
                    let start = *start as usize;
                    let end = start + len;
                    if end as usize > arrow_data_buf.len() {
                        return Err(::re_types_core::DeserializationError::offset_slice_oob(
                            (start, end),
                            arrow_data_buf.len(),
                        ));
                    }

                    #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                    let data = unsafe { arrow_data_buf.clone().sliced_unchecked(start, len) };
                    Ok(data)
                })
                .transpose()
            })
            .map(|res_or_opt| {
                res_or_opt.map(|res_or_opt| res_or_opt.map(|v| ::re_types_core::ArrowString(v)))
            })
            .collect::<::re_types_core::DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.testing.components.AffixFuzzer10#single_string_optional")?
            .into_iter()
        }
        .map(Ok)
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<::re_types_core::DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.testing.components.AffixFuzzer10#single_string_optional")
        .with_context("rerun.testing.components.AffixFuzzer10")?)
    }
}
