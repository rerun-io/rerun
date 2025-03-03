// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/recording_started_timestamp.fbs".

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

use crate::try_serialize_field;
use crate::SerializationResult;
use crate::{ComponentBatch, SerializedComponentBatch};
use crate::{ComponentDescriptor, ComponentName};
use crate::{DeserializationError, DeserializationResult};

/// **Component**: When the recording started.
///
/// Should be an absolute time, i.e. relative to Unix Epoch.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct RecordingStartedTimestamp(pub crate::datatypes::TimeInt);

impl crate::Component for RecordingStartedTimestamp {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.components.RecordingStartedTimestamp")
    }
}

crate::macros::impl_into_cow!(RecordingStartedTimestamp);

impl crate::Loggable for RecordingStartedTimestamp {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::TimeInt::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::TimeInt::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::datatypes::TimeInt::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }

    #[inline]
    fn from_arrow(arrow_data: &dyn arrow::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::TimeInt::from_arrow(arrow_data).map(|v| v.into_iter().map(Self).collect())
    }
}

impl<T: Into<crate::datatypes::TimeInt>> From<T> for RecordingStartedTimestamp {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::TimeInt> for RecordingStartedTimestamp {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::TimeInt {
        &self.0
    }
}

impl std::ops::Deref for RecordingStartedTimestamp {
    type Target = crate::datatypes::TimeInt;

    #[inline]
    fn deref(&self) -> &crate::datatypes::TimeInt {
        &self.0
    }
}

impl std::ops::DerefMut for RecordingStartedTimestamp {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::TimeInt {
        &mut self.0
    }
}

impl ::re_byte_size::SizeBytes for RecordingStartedTimestamp {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::TimeInt>::is_pod()
    }
}
