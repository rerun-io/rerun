// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/half_size3d.fbs".

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

use ::re_types_core::try_serialize_field;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: Half-size (radius) of a 3D box.
///
/// Measured in its local coordinate system.
///
/// The box extends both in negative and positive direction along each axis.
/// Negative sizes indicate that the box is flipped along the respective axis, but this has no effect on how it is displayed.
#[derive(Clone, Debug, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct HalfSize3D(pub crate::datatypes::Vec3D);

impl ::re_types_core::Component for HalfSize3D {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.components.HalfSize3D")
    }
}

::re_types_core::macros::impl_into_cow!(HalfSize3D);

impl ::re_types_core::Loggable for HalfSize3D {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::Vec3D::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::Vec3D::to_arrow_opt(data.into_iter().map(|datum| {
            datum.map(|datum| match datum.into() {
                ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&datum.0),
                ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(datum.0),
            })
        }))
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        crate::datatypes::Vec3D::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }

    #[inline]
    fn from_arrow(arrow_data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::Vec3D::from_arrow(arrow_data).map(bytemuck::cast_vec)
    }
}

impl<T: Into<crate::datatypes::Vec3D>> From<T> for HalfSize3D {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Vec3D> for HalfSize3D {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Vec3D {
        &self.0
    }
}

impl std::ops::Deref for HalfSize3D {
    type Target = crate::datatypes::Vec3D;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Vec3D {
        &self.0
    }
}

impl std::ops::DerefMut for HalfSize3D {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::Vec3D {
        &mut self.0
    }
}

impl ::re_byte_size::SizeBytes for HalfSize3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Vec3D>::is_pod()
    }
}
