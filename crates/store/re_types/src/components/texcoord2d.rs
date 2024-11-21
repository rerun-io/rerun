// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/texcoord2d.fbs".

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
    #[inline]
    fn arrow2_datatype() -> arrow2::datatypes::DataType {
        crate::datatypes::Vec2D::arrow2_datatype()
    }

    fn to_arrow2_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::Vec2D::to_arrow2_opt(data.into_iter().map(|datum| {
            datum.map(|datum| match datum.into() {
                ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&datum.0),
                ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(datum.0),
            })
        }))
    }

    fn from_arrow2_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        crate::datatypes::Vec2D::from_arrow2_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }

    #[inline]
    fn from_arrow2(arrow_data: &dyn arrow2::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::Vec2D::from_arrow2(arrow_data).map(bytemuck::cast_vec)
    }
}

impl ::re_types_core::Component for Texcoord2D {
    #[inline]
    fn name() -> ComponentName {
        "rerun.components.Texcoord2D".into()
    }
}
