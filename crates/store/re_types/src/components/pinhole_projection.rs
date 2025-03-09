// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/pinhole_projection.fbs".

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
use ::re_types_core::{ComponentBatch as _, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: Camera projection, from image coordinates to view coordinates.
///
/// Child from parent.
/// Image coordinates from camera view coordinates.
///
/// Example:
/// ```text
/// 1496.1     0.0  980.5
///    0.0  1496.1  744.5
///    0.0     0.0    1.0
/// ```
#[derive(Clone, Debug, Copy, PartialEq, PartialOrd)]
pub struct PinholeProjection(pub crate::datatypes::Mat3x3);

impl ::re_types_core::Component for PinholeProjection {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.components.PinholeProjection")
    }
}

::re_types_core::macros::impl_into_cow!(PinholeProjection);

impl ::re_types_core::Loggable for PinholeProjection {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::Mat3x3::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::Mat3x3::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::datatypes::Mat3x3::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }

    #[inline]
    fn from_arrow(arrow_data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::Mat3x3::from_arrow(arrow_data).map(|v| v.into_iter().map(Self).collect())
    }
}

impl<T: Into<crate::datatypes::Mat3x3>> From<T> for PinholeProjection {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Mat3x3> for PinholeProjection {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Mat3x3 {
        &self.0
    }
}

impl std::ops::Deref for PinholeProjection {
    type Target = crate::datatypes::Mat3x3;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Mat3x3 {
        &self.0
    }
}

impl std::ops::DerefMut for PinholeProjection {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::Mat3x3 {
        &mut self.0
    }
}

impl ::re_byte_size::SizeBytes for PinholeProjection {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Mat3x3>::is_pod()
    }
}
