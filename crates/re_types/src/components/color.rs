// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/color.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
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
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: An RGBA color with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.
///
/// The color is stored as a 32-bit integer, where the most significant
/// byte is `R` and the least significant byte is `A`.
#[derive(Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct Color(pub crate::datatypes::Rgba32);

impl ::re_types_core::SizeBytes for Color {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Rgba32>::is_pod()
    }
}

impl<T: Into<crate::datatypes::Rgba32>> From<T> for Color {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Rgba32> for Color {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Rgba32 {
        &self.0
    }
}

impl std::ops::Deref for Color {
    type Target = crate::datatypes::Rgba32;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Rgba32 {
        &self.0
    }
}

::re_types_core::macros::impl_into_cow!(Color);

impl ::re_types_core::Loggable for Color {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.Color".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::UInt32
    }

    #[allow(clippy::wildcard_imports)]
    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
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
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            PrimitiveArray::new(
                Self::arrow_datatype(),
                data0
                    .into_iter()
                    .map(|datum| datum.map(|datum| datum.0).unwrap_or_default())
                    .collect(),
                data0_bitmap,
            )
            .boxed()
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        Ok(arrow_data
            .as_any()
            .downcast_ref::<UInt32Array>()
            .ok_or_else(|| {
                let expected = Self::arrow_datatype();
                let actual = arrow_data.data_type().clone();
                DeserializationError::datatype_mismatch(expected, actual)
            })
            .with_context("rerun.components.Color#rgba")?
            .into_iter()
            .map(|opt| opt.copied())
            .map(|res_or_opt| res_or_opt.map(|v| crate::datatypes::Rgba32(v)))
            .map(|v| v.ok_or_else(DeserializationError::missing_data))
            .map(|res| res.map(|v| Some(Self(v))))
            .collect::<DeserializationResult<Vec<Option<_>>>>()
            .with_context("rerun.components.Color#rgba")
            .with_context("rerun.components.Color")?)
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn from_arrow(arrow_data: &dyn arrow2::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, buffer::*, datatypes::*};
        if let Some(validity) = arrow_data.validity() {
            if validity.unset_bits() != 0 {
                return Err(DeserializationError::missing_data());
            }
        }
        Ok({
            let slice = arrow_data
                .as_any()
                .downcast_ref::<UInt32Array>()
                .ok_or_else(|| {
                    let expected = DataType::UInt32;
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.components.Color#rgba")?
                .values()
                .as_slice();
            {
                slice
                    .iter()
                    .copied()
                    .map(|v| crate::datatypes::Rgba32(v))
                    .map(|v| Self(v))
                    .collect::<Vec<_>>()
            }
        })
    }
}
