// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/zoom_level.fbs".

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

/// **Component**: A zoom level determines how much of the world is visible on a map.
#[derive(Clone, Debug, Default)]
pub struct ZoomLevel(
    /// Zoom level: 0 being the lowest zoom level (fully zoomed out) and 22 being the highest (fully zoomed in).
    pub crate::datatypes::Float64,
);

impl ::re_types_core::Component for ZoomLevel {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.blueprint.components.ZoomLevel")
    }
}

::re_types_core::macros::impl_into_cow!(ZoomLevel);

impl ::re_types_core::Loggable for ZoomLevel {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::Float64::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::Float64::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::datatypes::Float64::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }

    #[inline]
    fn from_arrow(arrow_data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::Float64::from_arrow(arrow_data).map(|v| v.into_iter().map(Self).collect())
    }
}

impl<T: Into<crate::datatypes::Float64>> From<T> for ZoomLevel {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Float64> for ZoomLevel {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Float64 {
        &self.0
    }
}

impl std::ops::Deref for ZoomLevel {
    type Target = crate::datatypes::Float64;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Float64 {
        &self.0
    }
}

impl std::ops::DerefMut for ZoomLevel {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::Float64 {
        &mut self.0
    }
}

impl ::re_byte_size::SizeBytes for ZoomLevel {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Float64>::is_pod()
    }
}
