// DO NOT EDIT!: This file was autogenerated by re_types_builder in crates/re_types_builder/src/codegen/rust/api.rs:165

#![allow(trivial_numeric_casts)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

/// A String label component.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Label(pub crate::datatypes::Label);

impl<T: Into<crate::datatypes::Label>> From<T> for Label {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl<'a> From<Label> for ::std::borrow::Cow<'a, Label> {
    #[inline]
    fn from(value: Label) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Label> for ::std::borrow::Cow<'a, Label> {
    #[inline]
    fn from(value: &'a Label) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for Label {
    type Name = crate::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.label".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Utf8
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, datatypes::*};
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
            let data0_bitmap: Option<::arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                let inner_data: ::arrow2::buffer::Buffer<u8> = data0
                    .iter()
                    .flatten()
                    .flat_map(|datum| {
                        let crate::datatypes::Label(data0) = datum;
                        data0.0.clone()
                    })
                    .collect();
                let offsets =
                    ::arrow2::offset::Offsets::<i32>::try_from_lengths(data0.iter().map(|opt| {
                        opt.as_ref()
                            .map(|datum| {
                                let crate::datatypes::Label(data0) = datum;
                                data0.0.len()
                            })
                            .unwrap_or_default()
                    }))
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
    fn try_from_arrow_opt(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        Ok({
            let arrow_data = arrow_data
                .as_any()
                .downcast_ref::<::arrow2::array::Utf8Array<i32>>()
                .ok_or_else(|| {
                    crate::DeserializationError::datatype_mismatch(
                        DataType::Utf8,
                        arrow_data.data_type().clone(),
                    )
                })
                .with_context("rerun.components.Label#value")?;
            let arrow_data_buf = arrow_data.values();
            let offsets = arrow_data.offsets();
            arrow2::bitmap::utils::ZipValidity::new_with_validity(
                offsets.iter().zip(offsets.lengths()),
                arrow_data.validity(),
            )
            .map(|elem| {
                elem.map(|(start, len)| {
                    let start = *start as usize;
                    let end = start + len;
                    if end as usize > arrow_data_buf.len() {
                        return Err(crate::DeserializationError::offset_slice_oob(
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
                    res_or_opt.map(|v| crate::datatypes::Label(crate::ArrowString(v)))
                })
            })
            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.Label#value")?
            .into_iter()
        }
        .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
        .map(|res| res.map(|v| Some(Self(v))))
        .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
        .with_context("rerun.components.Label#value")
        .with_context("rerun.components.Label")?)
    }
}
