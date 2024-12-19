// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/filter_by_range.fbs".

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
use ::re_types_core::{ComponentBatch, ComponentBatchCowWithDescriptor};
use ::re_types_core::{ComponentDescriptor, ComponentName};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **Component**: Configuration for a filter-by-range feature of the dataframe view.
#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct FilterByRange(pub crate::blueprint::datatypes::FilterByRange);

impl ::re_types_core::Component for FilterByRange {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.blueprint.components.FilterByRange")
    }
}

::re_types_core::macros::impl_into_cow!(FilterByRange);

impl ::re_types_core::Loggable for FilterByRange {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::blueprint::datatypes::FilterByRange::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::blueprint::datatypes::FilterByRange::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::blueprint::datatypes::FilterByRange::from_arrow2_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }
}

impl<T: Into<crate::blueprint::datatypes::FilterByRange>> From<T> for FilterByRange {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::blueprint::datatypes::FilterByRange> for FilterByRange {
    #[inline]
    fn borrow(&self) -> &crate::blueprint::datatypes::FilterByRange {
        &self.0
    }
}

impl std::ops::Deref for FilterByRange {
    type Target = crate::blueprint::datatypes::FilterByRange;

    #[inline]
    fn deref(&self) -> &crate::blueprint::datatypes::FilterByRange {
        &self.0
    }
}

impl std::ops::DerefMut for FilterByRange {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::blueprint::datatypes::FilterByRange {
        &mut self.0
    }
}

impl ::re_byte_size::SizeBytes for FilterByRange {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::datatypes::FilterByRange>::is_pod()
    }
}
