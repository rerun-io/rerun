// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/texcoord2d.fbs".

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

/// **Component**: A 2D texture UV coordinate.
///
/// Texture coordinates specify a position on a 2D texture.
/// A range from 0-1 covers the entire texture in the respective dimension.
/// Unless configured otherwise, the texture repeats outside of this range.
/// Rerun uses top-left as the origin for UV coordinates.
///
///   0     U     1
/// 0 + --------- →
///   |           .
/// V |           .
///   |           .
/// 1 ↓ . . . . . .
///
/// This is the same convention as in Vulkan/Metal/DX12/WebGPU, but (!) unlike OpenGL,
/// which places the origin at the bottom-left.
#[derive(Clone, Debug, Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct Texcoord2D(pub crate::datatypes::Vec2D);

impl ::re_types_core::SizeBytes for Texcoord2D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Vec2D>::is_pod()
    }
}

impl<T: Into<crate::datatypes::Vec2D>> From<T> for Texcoord2D {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Vec2D> for Texcoord2D {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Vec2D {
        &self.0
    }
}

impl std::ops::Deref for Texcoord2D {
    type Target = crate::datatypes::Vec2D;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Vec2D {
        &self.0
    }
}

impl std::ops::DerefMut for Texcoord2D {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::Vec2D {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(Texcoord2D);

impl ::re_types_core::Loggable for Texcoord2D {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.Texcoord2D".into()
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        use arrow2::datatypes::*;
        DataType::FixedSizeList(
            std::sync::Arc::new(Field::new("item", DataType::Float32, false)),
            2usize,
        )
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
                    let datum = datum.map(|datum| datum.into_owned().0);
                    (datum.is_some(), datum)
                })
                .unzip();
            let data0_bitmap: Option<arrow2::bitmap::Bitmap> = {
                let any_nones = somes.iter().any(|some| !*some);
                any_nones.then(|| somes.into())
            };
            {
                use arrow2::{buffer::Buffer, offset::OffsetsBuffer};
                let data0_inner_data: Vec<_> = data0
                    .into_iter()
                    .map(|datum| datum.map(|datum| datum.0).unwrap_or_default())
                    .flatten()
                    .collect();
                let data0_inner_bitmap: Option<arrow2::bitmap::Bitmap> =
                    data0_bitmap.as_ref().map(|bitmap| {
                        bitmap
                            .iter()
                            .map(|b| std::iter::repeat(b).take(2usize))
                            .flatten()
                            .collect::<Vec<_>>()
                            .into()
                    });
                FixedSizeListArray::new(
                    Self::arrow_datatype(),
                    PrimitiveArray::new(
                        DataType::Float32,
                        data0_inner_data.into_iter().collect(),
                        data0_inner_bitmap,
                    )
                    .boxed(),
                    data0_bitmap,
                )
                .boxed()
            }
        })
    }

    #[allow(clippy::wildcard_imports)]
    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        crate::datatypes::Vec2D::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(|v| Self(v))).collect())
    }

    #[allow(clippy::wildcard_imports)]
    #[inline]
    fn from_arrow(arrow_data: &dyn arrow2::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::Vec2D::from_arrow(arrow_data)
            .map(|v| v.into_iter().map(|v| Self(v)).collect())
    }
}
