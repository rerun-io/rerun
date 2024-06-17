// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/re_types/definitions/rerun/blueprint/views/tensor.fbs".

#![allow(trivial_numeric_casts)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::cloned_instead_of_copied)]
#![allow(clippy::iter_on_single_items)]
#![allow(clippy::map_flatten)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::new_without_default)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_cast)]

use ::re_types_core::external::arrow2;
use ::re_types_core::ComponentName;
use ::re_types_core::SerializationResult;
use ::re_types_core::{ComponentBatch, MaybeOwnedComponentBatch};
use ::re_types_core::{DeserializationError, DeserializationResult};

/// **View**: A view on a tensor of any dimensionality.
#[derive(Clone, Debug)]
pub struct TensorView {
    /// Configures how scalars are mapped to color.
    pub colormap: crate::blueprint::archetypes::ScalarColormap,

    /// Configures how the selected slice is displayed.
    pub filter: crate::blueprint::archetypes::TensorSliceFilter,
}

impl ::re_types_core::SizeBytes for TensorView {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.colormap.heap_size_bytes() + self.filter.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::archetypes::ScalarColormap>::is_pod()
            && <crate::blueprint::archetypes::TensorSliceFilter>::is_pod()
    }
}

impl ::re_types_core::View for TensorView {
    #[inline]
    fn identifier() -> ::re_types_core::SpaceViewClassIdentifier {
        "Tensor".into()
    }
}
