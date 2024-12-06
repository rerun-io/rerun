// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/plane3d.fbs".

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

/// **Component**: An infinite 3D plane represented by a unit normal vector and a distance.
///
/// Any point P on the plane fulfills the equation `dot(xyz, P) - d = 0`,
/// where `xyz` is the plane's normal and `d` the distance of the plane from the origin.
/// This representation is also known as the Hesse normal form.
///
/// Note: although the normal will be passed through to the
/// datastore as provided, when used in the Viewer, planes will always be normalized.
/// I.e. the plane with xyz = (2, 0, 0), d = 1 is equivalent to xyz = (1, 0, 0), d = 0.5
#[derive(Clone, Debug, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(transparent)]
pub struct Plane3D(pub crate::datatypes::Plane3D);

impl ::re_types_core::SizeBytes for Plane3D {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Plane3D>::is_pod()
    }
}

impl<T: Into<crate::datatypes::Plane3D>> From<T> for Plane3D {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Plane3D> for Plane3D {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Plane3D {
        &self.0
    }
}

impl std::ops::Deref for Plane3D {
    type Target = crate::datatypes::Plane3D;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Plane3D {
        &self.0
    }
}

impl std::ops::DerefMut for Plane3D {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::Plane3D {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(Plane3D);

impl ::re_types_core::Loggable for Plane3D {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::Plane3D::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::Plane3D::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::datatypes::Plane3D::from_arrow2_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }

    #[inline]
    fn from_arrow2(arrow_data: &dyn arrow2::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::Plane3D::from_arrow2(arrow_data).map(bytemuck::cast_vec)
    }
}

impl ::re_types_core::Component for Plane3D {
    #[inline]
    fn name() -> ComponentName {
        "rerun.components.Plane3D".into()
    }
}
