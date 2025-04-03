// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/selected_columns.fbs".

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

/// **Component**: Describe a component column to be selected in the dataframe view.
///
/// ⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct SelectedColumns(pub crate::blueprint::datatypes::SelectedColumns);

impl ::re_types_core::Component for SelectedColumns {
    #[inline]
    fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor::new("rerun.blueprint.components.SelectedColumns")
    }
}

::re_types_core::macros::impl_into_cow!(SelectedColumns);

impl ::re_types_core::Loggable for SelectedColumns {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::blueprint::datatypes::SelectedColumns::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<arrow::array::ArrayRef>
    where
        Self: Clone + 'a,
    {
        crate::blueprint::datatypes::SelectedColumns::to_arrow_opt(data.into_iter().map(|datum| {
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
        crate::blueprint::datatypes::SelectedColumns::from_arrow_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }
}

impl<T: Into<crate::blueprint::datatypes::SelectedColumns>> From<T> for SelectedColumns {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::blueprint::datatypes::SelectedColumns> for SelectedColumns {
    #[inline]
    fn borrow(&self) -> &crate::blueprint::datatypes::SelectedColumns {
        &self.0
    }
}

impl std::ops::Deref for SelectedColumns {
    type Target = crate::blueprint::datatypes::SelectedColumns;

    #[inline]
    fn deref(&self) -> &crate::blueprint::datatypes::SelectedColumns {
        &self.0
    }
}

impl std::ops::DerefMut for SelectedColumns {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::blueprint::datatypes::SelectedColumns {
        &mut self.0
    }
}

impl ::re_byte_size::SizeBytes for SelectedColumns {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::datatypes::SelectedColumns>::is_pod()
    }
}
