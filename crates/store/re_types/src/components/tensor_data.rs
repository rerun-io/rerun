// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/tensor_data.fbs".

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
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: An N-dimensional array of numbers.
///
/// The number of dimensions and their respective lengths is specified by the `shape` field.
/// The dimensions are ordered from outermost to innermost. For example, in the common case of
/// a 2D RGB Image, the shape would be `[height, width, channel]`.
///
/// These dimensions are combined with an index to look up values from the `buffer` field,
/// which stores a contiguous array of typed values.
#[derive(Clone, Debug, Default, PartialEq)]
#[repr(transparent)]
pub struct TensorData(pub crate::datatypes::TensorData);

impl ::re_types_core::Component for TensorData {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.components.TensorData")
    }
}

::re_types_core::macros::impl_into_cow!(TensorData);

impl ::re_types_core::Loggable for TensorData {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::TensorData::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::TensorData::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::datatypes::TensorData::from_arrow2_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }
}

impl<T: Into<crate::datatypes::TensorData>> From<T> for TensorData {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::TensorData> for TensorData {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::TensorData {
        &self.0
    }
}

impl std::ops::Deref for TensorData {
    type Target = crate::datatypes::TensorData;

    #[inline]
    fn deref(&self) -> &crate::datatypes::TensorData {
        &self.0
    }
}

impl std::ops::DerefMut for TensorData {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::TensorData {
        &mut self.0
    }
}

impl ::re_types_core::SizeBytes for TensorData {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::TensorData>::is_pod()
    }
}
