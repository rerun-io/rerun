// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/color.fbs".

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

/// An RGBA color with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.
///
/// The color is stored as a 32-bit integer, where the most significant
/// byte is `R` and the least significant byte is `A`.
#[derive(
    Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, bytemuck :: Pod, bytemuck :: Zeroable,
)]
#[repr(transparent)]
pub struct Color(pub crate::datatypes::Color);

impl<T: Into<crate::datatypes::Color>> From<T> for Color {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Color> for Color {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Color {
        &self.0
    }
}

impl std::ops::Deref for Color {
    type Target = crate::datatypes::Color;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Color {
        &self.0
    }
}

impl<'a> From<Color> for ::std::borrow::Cow<'a, Color> {
    #[inline]
    fn from(value: Color) -> Self {
        std::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a Color> for ::std::borrow::Cow<'a, Color> {
    #[inline]
    fn from(value: &'a Color) -> Self {
        std::borrow::Cow::Borrowed(value)
    }
}

impl crate::Loggable for Color {
    type Name = crate::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.colorrgba".into()
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use ::arrow2::datatypes::*;
        DataType::UInt32
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
            PrimitiveArray::new(
                Self::arrow_datatype(),
                data0
                    .into_iter()
                    .map(|datum| {
                        datum
                            .map(|datum| {
                                let crate::datatypes::Color(data0) = datum;
                                data0
                            })
                            .unwrap_or_default()
                    })
                    .collect(),
                data0_bitmap,
            )
            .boxed()
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
        Ok(arrow_data
            .as_any()
            .downcast_ref::<UInt32Array>()
            .ok_or_else(|| {
                crate::DeserializationError::datatype_mismatch(
                    DataType::UInt32,
                    arrow_data.data_type().clone(),
                )
            })
            .with_context("rerun.components.Color#rgba")?
            .into_iter()
            .map(|opt| opt.copied())
            .map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::Color(v)))
            .map(|v| v.ok_or_else(crate::DeserializationError::missing_data))
            .map(|res| res.map(|v| Some(Self(v))))
            .collect::<crate::DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.Color#rgba")
            .with_context("rerun.components.Color")?)
    }

    #[allow(unused_imports, clippy::wildcard_imports)]
    #[inline]
    fn try_from_arrow(
        arrow_data: &dyn ::arrow2::array::Array,
    ) -> crate::DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        use crate::{Loggable as _, ResultExt as _};
        use ::arrow2::{array::*, buffer::*, datatypes::*};
        if let Some(validity) = arrow_data.validity() {
            if validity.unset_bits() != 0 {
                return Err(crate::DeserializationError::missing_data());
            }
        }
        Ok(arrow_data
            .as_any()
            .downcast_ref::<UInt32Array>()
            .ok_or_else(|| {
                crate::DeserializationError::datatype_mismatch(
                    DataType::UInt32,
                    arrow_data.data_type().clone(),
                )
            })
            .with_context("rerun.components.Color#rgba")?
            .values()
            .as_slice()
            .iter()
            .copied()
            .map(|v| crate::datatypes::Color(v))
            .map(|v| Self(v))
            .collect::<Vec<_>>())
    }
}
