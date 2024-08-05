// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/pixel_format.fbs".

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

/// **Component**: Specifieds a particular format of an [`archetypes::Image`][crate::archetypes::Image].
///
/// Most images can be described by a [`components::ColorModel`][crate::components::ColorModel] and a [`components::ChannelDatatype`][crate::components::ChannelDatatype],
/// e.g. `RGB` and `U8` respectively.
///
/// However, some image formats has chroma downsampling and/or
/// use differing number of bits per channel, and that is what this [`components::PixelFormat`][crate::components::PixelFormat] is for.
///
/// All these formats support random access.
///
/// For more compressed image formats, see [`archetypes::ImageEncoded`][crate::archetypes::ImageEncoded].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Default)]
pub enum PixelFormat {
    /// NV12 (aka Y_UV12) is a YUV 4:2:0 chroma downsampled format with 12 bits per pixel and 8 bits per channel.
    ///
    /// First comes entire image in Y in one plane,
    /// followed by a plane with interleaved lines ordered as U0, V0, U1, V1, etc.
    #[default]
    #[allow(clippy::upper_case_acronyms)]
    NV12 = 1,

    /// YUY2 (aka YUYV or YUYV16), is a YUV 4:2:2 chroma downsampled format with 16 bits per pixel and 8 bits per channel.
    ///
    /// The order of the channels is Y0, U0, Y1, V0, all in the same plane.
    #[allow(clippy::upper_case_acronyms)]
    YUY2 = 2,
}

impl ::re_types_core::reflection::Enum for PixelFormat {
    #[inline]
    fn variants() -> &'static [Self] {
        &[Self::NV12, Self::YUY2]
    }

    #[inline]
    fn docstring_md(self) -> &'static str {
        match self {
            Self::NV12 => {
                "NV12 (aka Y_UV12) is a YUV 4:2:0 chroma downsampled format with 12 bits per pixel and 8 bits per channel.\n\nFirst comes entire image in Y in one plane,\nfollowed by a plane with interleaved lines ordered as U0, V0, U1, V1, etc."
            }
            Self::YUY2 => {
                "YUY2 (aka YUYV or YUYV16), is a YUV 4:2:2 chroma downsampled format with 16 bits per pixel and 8 bits per channel.\n\nThe order of the channels is Y0, U0, Y1, V0, all in the same plane."
            }
        }
    }
}

impl ::re_types_core::SizeBytes for PixelFormat {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl std::fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NV12 => write!(f, "NV12"),
            Self::YUY2 => write!(f, "YUY2"),
        }
    }
}

::re_types_core::macros::impl_into_cow!(PixelFormat);

impl ::re_types_core::Loggable for PixelFormat {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.PixelFormat".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        #![allow(clippy::wildcard_imports)]
        use arrow2::datatypes::*;
        DataType::Union(
            std::sync::Arc::new(vec![
                Field::new("_null_markers", DataType::Null, true),
                Field::new("NV12", DataType::Null, true),
                Field::new("YUY2", DataType::Null, true),
            ]),
            Some(std::sync::Arc::new(vec![0i32, 1i32, 2i32])),
            UnionMode::Sparse,
        )
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        #![allow(clippy::wildcard_imports)]
        use ::re_types_core::{Loggable as _, ResultExt as _};
        use arrow2::{array::*, datatypes::*};
        Ok({
            // Sparse Arrow union
            let data: Vec<_> = data
                .into_iter()
                .map(|datum| {
                    let datum: Option<::std::borrow::Cow<'a, Self>> = datum.map(Into::into);
                    datum
                })
                .collect();
            let num_variants = 2usize;
            let types = data
                .iter()
                .map(|a| match a.as_deref() {
                    None => 0,
                    Some(value) => *value as i8,
                })
                .collect();
            let fields: Vec<_> =
                std::iter::repeat(NullArray::new(DataType::Null, data.len()).boxed())
                    .take(1 + num_variants)
                    .collect();
            UnionArray::new(Self::arrow_datatype(), types, fields, None).boxed()
        })
    }

    fn from_arrow_opt(
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
                .downcast_ref::<arrow2::array::UnionArray>()
                .ok_or_else(|| {
                    let expected = Self::arrow_datatype();
                    let actual = arrow_data.data_type().clone();
                    DeserializationError::datatype_mismatch(expected, actual)
                })
                .with_context("rerun.components.PixelFormat")?;
            let arrow_data_types = arrow_data.types();
            arrow_data_types
                .iter()
                .map(|typ| match typ {
                    0 => Ok(None),
                    1 => Ok(Some(Self::NV12)),
                    2 => Ok(Some(Self::YUY2)),
                    _ => Err(DeserializationError::missing_union_arm(
                        Self::arrow_datatype(),
                        "<invalid>",
                        *typ as _,
                    )),
                })
                .collect::<DeserializationResult<Vec<_>>>()
                .with_context("rerun.components.PixelFormat")?
        })
    }
}
