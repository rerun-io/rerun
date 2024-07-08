// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/components/text_log_level.fbs".

#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::map_flatten)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_lines)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: The severity level of a text log message.
///
/// Recommended to be one of:
/// * `"CRITICAL"`
/// * `"ERROR"`
/// * `"WARN"`
/// * `"INFO"`
/// * `"DEBUG"`
/// * `"TRACE"`
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TextLogLevel(pub crate::datatypes::Utf8);

impl ::re_types_core::SizeBytes for TextLogLevel {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::Utf8>::is_pod()
    }
}

impl<T: Into<crate::datatypes::Utf8>> From<T> for TextLogLevel {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::Utf8> for TextLogLevel {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::Utf8 {
        &self.0
    }
}

impl std::ops::Deref for TextLogLevel {
    type Target = crate::datatypes::Utf8;

    #[inline]
    fn deref(&self) -> &crate::datatypes::Utf8 {
        &self.0
    }
}

impl std::ops::DerefMut for TextLogLevel {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::Utf8 {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(TextLogLevel);

impl ::re_types_core::Loggable for TextLogLevel {
    type Name = ::re_types_core::ComponentName;

    #[inline]
    fn name() -> Self::Name {
        "rerun.components.TextLogLevel".into()
    }

    #[inline]
    fn arrow_datatype() -> arrow2::datatypes::DataType {
        crate::datatypes::Utf8::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::Utf8::to_arrow_opt(data.into_iter().map(|datum| {
            datum.map(|datum| match datum.into() {
                ::std::borrow::Cow::Borrowed(datum) => ::std::borrow::Cow::Borrowed(&datum.0),
                ::std::borrow::Cow::Owned(datum) => ::std::borrow::Cow::Owned(datum.0),
            })
        }))
    }

    fn from_arrow_opt(
        arrow_data: &dyn arrow2::array::Array,
    ) -> DeserializationResult<Vec<Option<Self>>>
    where
        Self: Sized,
    {
        crate::datatypes::Utf8::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }
}
