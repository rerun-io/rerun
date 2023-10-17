// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/media_type.fbs".

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

/// **Component**: A standardized media type (RFC2046, formerly known as MIME types), encoded as a utf8 string.
///
/// The complete reference of officially registered media types is maintained by the IANA and can be
/// consulted at <https://www.iana.org/assignments/media-types/media-types.xhtml>.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct MediaType(pub crate::datatypes::Utf8);

impl<T: Into<crate::datatypes::Utf8>> From<T> for MediaType {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Utf8> for MediaType {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Utf8 {
        &self.0
    }
}

impl std::ops::Deref for MediaType {
    type Target = crate::datatypes::Utf8;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Utf8 {
        &self.0
    }
}

impl<'a> From<MediaType> for ::std::borrow::Cow<'a, MediaType> {
    #[inline]
    fn from(value: MediaType) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a MediaType> for ::std::borrow::Cow<'a, MediaType> {
    #[inline]
    fn from(value: &'a MediaType) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl ::re_types_core::Loggable for MediaType {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.MediaType".into()
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
                    let datum = datum.map(|datum| {
                        let Self(data0) = datum.into_owned();
                        data0
                    });
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<::re_types_core::external::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                let inner_data: ::re_types_core::external::arrow2::buffer::Buffer<u8> = data0
                    .iter()
                    .flatten()
                    .flat_map(|datum| {
                        let crate::datatypes::Utf8(data0) = datum;
                        data0.0.clone()
                    })
                    .collect();
                let offsets =
                    ::re_types_core::external::arrow2::offset::Offsets::<i32>::try_from_lengths(
                        data0.iter().map(|opt| {
                            opt.as_ref()
                                .map(|datum| {
                                    let crate::datatypes::Utf8(data0) = datum;
                                    data0.0.len()
                                })
                                .unwrap_or_default()
                        }),
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
                .with_context("rerun.components.MediaType#value")?;
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
                res_or_opt.map(|res_or_opt| {
                    res_or_opt.map(|v| crate::datatypes::Utf8(::re_types_core::ArrowString(v)))
                })
            })
            .collect::<::re_types_core::DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.MediaType#value")?
            .into_iter()
        }
        .map(|v| v.ok_or_else(::re_types_core::DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<::re_types_core::DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.components.MediaType#value")
        .with_context("rerun.components.MediaType")?)
    }
}
