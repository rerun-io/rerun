// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/visualizer_overrides.fbs".

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

/// **Component**: Override the visualizers for an entity.
///
/// This component is a stop-gap mechanism based on the current implementation details
/// of the visualizer system. It is not intended to be a long-term solution, but provides
/// enough utility to be useful in the short term.
///
/// The long-term solution is likely to be based off: <https://github.com/rerun-io/rerun/issues/6626>
///
/// This can only be used as part of blueprints. It will have no effect if used
/// in a regular entity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct VisualizerOverrides(
    /// Names of the visualizers that should be active.
    pub crate::blueprint::datatypes::Utf8List,
);

impl ::re_types_core::Component for VisualizerOverrides {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.blueprint.components.VisualizerOverrides")
    }
}

::re_types_core::macros::impl_into_cow!(VisualizerOverrides);

impl ::re_types_core::Loggable for VisualizerOverrides {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::blueprint::datatypes::Utf8List::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::blueprint::datatypes::Utf8List::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::blueprint::datatypes::Utf8List::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }
}

impl<T: Into<crate::blueprint::datatypes::Utf8List>> From<T> for VisualizerOverrides {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::blueprint::datatypes::Utf8List> for VisualizerOverrides {
    #[inline]
    fn borrow(&self) -> &crate::blueprint::datatypes::Utf8List {
        &self.0
    }
}

impl std::ops::Deref for VisualizerOverrides {
    type Target = crate::blueprint::datatypes::Utf8List;

    #[inline]
    fn deref(&self) -> &crate::blueprint::datatypes::Utf8List {
        &self.0
    }
}

impl std::ops::DerefMut for VisualizerOverrides {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::blueprint::datatypes::Utf8List {
        &mut self.0
    }
}

impl ::re_byte_size::SizeBytes for VisualizerOverrides {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::datatypes::Utf8List>::is_pod()
    }
}
