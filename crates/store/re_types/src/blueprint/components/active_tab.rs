// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/components/active_tab.fbs".

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

/// **Component**: The active tab in a tabbed container.
#[derive(Clone, Debug, Default)]
pub struct ActiveTab(
    /// Which tab is currently active.
    ///
    /// This should always correspond to a tab in the container.
    pub crate::datatypes::EntityPath,
);

impl ::re_types_core::SizeBytes for ActiveTab {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::datatypes::EntityPath>::is_pod()
    }
}

impl<T: Into<crate::datatypes::EntityPath>> From<T> for ActiveTab {
    fn from(v: T) -> Self {
        Self(v.into())
    }
}

impl std::borrow::Borrow<crate::datatypes::EntityPath> for ActiveTab {
    #[inline]
    fn borrow(&self) -> &crate::datatypes::EntityPath {
        &self.0
    }
}

impl std::ops::Deref for ActiveTab {
    type Target = crate::datatypes::EntityPath;

    #[inline]
    fn deref(&self) -> &crate::datatypes::EntityPath {
        &self.0
    }
}

impl std::ops::DerefMut for ActiveTab {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::datatypes::EntityPath {
        &mut self.0
    }
}

::re_types_core::macros::impl_into_cow!(ActiveTab);

impl ::re_types_core::Loggable for ActiveTab {
    #[inline]
    fn arrow_datatype() -> arrow::datatypes::DataType {
        crate::datatypes::EntityPath::arrow_datatype()
    }

    fn to_arrow2_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<::std::borrow::Cow<'a, Self>>>>,
    ) -> SerializationResult<Box<dyn arrow2::array::Array>>
    where
        Self: Clone + 'a,
    {
        crate::datatypes::EntityPath::to_arrow2_opt(data.into_iter().map(|datum| {
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
        crate::datatypes::EntityPath::from_arrow2_opt(arrow_data)
            .map(|v| v.into_iter().map(|v| v.map(Self)).collect())
    }
}

impl ::re_types_core::Component for ActiveTab {
    #[inline]
    fn name() -> ComponentName {
        "rerun.blueprint.components.ActiveTab".into()
    }
}
