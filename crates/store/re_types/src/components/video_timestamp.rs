// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/components/video_timestamp.fbs".

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

/// **Component**: Timestamp inside a [`archetypes::AssetVideo`][crate::archetypes::AssetVideo].
#[derive(Clone, Debug, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct VideoTimestamp(pub crate::datatypes::VideoTimestamp);

impl ::re_types_core::SizeBytes for VideoTimestamp {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::VideoTimestamp>::is_pod()
    }
}

impl<T: Into<crate::datatypes::VideoTimestamp>> From<T> for VideoTimestamp {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::VideoTimestamp> for VideoTimestamp {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::VideoTimestamp {
        &self.0
    }
}

impl std::ops::Deref for VideoTimestamp {
    type Target = crate::datatypes::VideoTimestamp;

    #[inline]
    fn deref(&self) -> &crate::datatypes::VideoTimestamp {
        &self.0
    }
}

impl std::ops::DerefMut for VideoTimestamp {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::VideoTimestamp {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(VideoTimestamp);

impl ::re_types_core::Loggable for VideoTimestamp {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::VideoTimestamp::arrow_datatype()
    }

    fn to_arrow2_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::VideoTimestamp::to_arrow2_opt(data.into_iter().map(|datum| {
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
        crate::datatypes::VideoTimestamp::from_arrow2_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }

    #[inline]
    fn from_arrow2(arrow_data: &dyn arrow2::array::Array) -> DeserializationResult<Vec<Self>>
    where
        Self: Sized,
    {
        crate::datatypes::VideoTimestamp::from_arrow2(arrow_data)
            .map(|v| v.into_iter().map(Self).collect())
    }
}

impl ::re_types_core::Component for VideoTimestamp {
    #[inline]
    fn name() -> ComponentName {
        "rerun.components.VideoTimestamp".into()
    }
}
