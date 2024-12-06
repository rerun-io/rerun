// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/views/dataframe.fbs".

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

/// **View**: A view to display any data in a tabular form.
///
/// Any data from the store can be shown, using a flexibly, user-configurable query.
#[derive(Clone, Debug)]
pub struct DataframeView {
    /// Query of the dataframe.
    pub query: crate::blueprint::archetypes::DataframeQuery,
}

impl ::re_types_core::View for DataframeView {
    #[inline]
    fn identifier() -> ::re_types_core::SpaceViewClassIdentifier {
        "Dataframe".into()
    }
}

impl<T: Into<crate::blueprint::archetypes::DataframeQuery>> From<T> for DataframeView {
    fn from(v: T) -> Self {
        Self { query: v.into() }
    }
}

impl std::borrow::Borrow<crate::blueprint::archetypes::DataframeQuery> for DataframeView {
    #[inline]
    fn borrow(&self) -> &crate::blueprint::archetypes::DataframeQuery {
        &self.query
    }
}

impl std::ops::Deref for DataframeView {
    type Target = crate::blueprint::archetypes::DataframeQuery;

    #[inline]
    fn deref(&self) -> &crate::blueprint::archetypes::DataframeQuery {
        &self.query
    }
}

impl std::ops::DerefMut for DataframeView {
    #[inline]
    fn deref_mut(&mut self) -> &mut crate::blueprint::archetypes::DataframeQuery {
        &mut self.query
    }
}

impl ::re_types_core::SizeBytes for DataframeView {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.query.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::archetypes::DataframeQuery>::is_pod()
    }
}
