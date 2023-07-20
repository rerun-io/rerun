// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

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
pub struct Label(pub String);

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
    type Iter<'a, I> = Box<dyn Iterator<Item = I> + 'a>;
    #[inline]
    fn name() -> Self::Name {
        "rerun.label".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn to_arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::Utf8
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    fn try_to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
        extension_wrapper: Option<&str>,
    ) -> crate::SerializationResult<Box<dyn ::arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        use crate::Loggable as _;
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
                let inner_data: ::arrow2::buffer::Buffer<u8> =
                    data0.iter().flatten().flat_map(|s| s.bytes()).collect();
                let offsets = ::arrow2::offset::Offsets::<i32>::try_from_lengths(
                    data0
                        .iter()
                        .map(|opt| opt.as_ref().map(|datum| datum.len()).unwrap_or_default()),
                )
                .unwrap()
                .into();
                #[allow(unsafe_code, clippy::undocumented_unsafe_blocks)]
                unsafe {
                    Utf8Array::<i32>::new_unchecked(
                        {
                            _ = extension_wrapper;
                            DataType::Extension(
                                "rerun.components.Label".to_owned(),
                                Box::new(DataType::Utf8),
                                None,
                            )
                            .to_logical_type()
                            .clone()
                        },
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
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use crate::Loggable as _;
        use ::arrow2::{array::*, datatypes::*};
        Ok(data
            .as_any()
            .downcast_ref::<Utf8Array<i32>>()
            .unwrap()
            .into_iter()
            .map(|v| v.map(ToOwned::to_owned))
            .map(|v| {
                v.ok_or_else(|| crate::DeserializationError::MissingData {
                    backtrace: ::backtrace::Backtrace::new_unresolved(),
                })
            })
            .map(|res| res.map(|v| Some(Self(v))))
            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
            .map_err(|err| crate::DeserializationError::Context {
                location: "rerun.components.Label#value".into(),
                source: Box::new(err),
            })?)
    }

    fn try_from_arrow_iter(
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Self::Iter<'_, Self>> {
        Ok(Box::new(Self::try_from_arrow(data)?.into_iter()))
    }

    fn try_from_arrow_opt_iter(
        data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Self::Iter<'_, Option<Self>>>
    where
        Self: Sized,
    {
        Ok(Box::new(Self::try_from_arrow_opt(data)?.into_iter()))
    }
}

impl crate::Component for Label {}
