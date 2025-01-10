// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/draw_order.fbs".

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
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor, SerializedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: Draw order of 2D elements. Higher values are drawn on top of lower values.
///
/// An entity can have only a single draw order component.
/// Within an entity draw order is governed by the order of the components.
///
/// Draw order for entities with the same draw order is generally undefined.
#[derive(Clone, Debug, Copy)]
#[repr(transparent)]
pub struct DrawOrder(pub crate::datatypes::Float32);

impl ::re_types_core::Component for DrawOrder {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.components.DrawOrder")
    }
}

::re_types_core::macros::impl_into_cow!(DrawOrder);

impl ::re_types_core::Loggable for DrawOrder {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::Float32::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::Float32::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::datatypes::Float32::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }

    #[inline]
    fn from_arrow(arrow_data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::Float32::from_arrow(arrow_data).map(|v| v.into_iter().map(Self).collect())
    }
}

impl<T: Into<crate::datatypes::Float32>> From<T> for DrawOrder {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Float32> for DrawOrder {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Float32 {
        &self.0
    }
}

impl std::ops::Deref for DrawOrder {
    type Target = crate::datatypes::Float32;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Float32 {
        &self.0
    }
}

impl std::ops::DerefMut for DrawOrder {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::Float32 {
        &mut self.0
    }
}

impl ::re_byte_size::SizeBytes for DrawOrder {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Float32>::is_pod()
    }
}
