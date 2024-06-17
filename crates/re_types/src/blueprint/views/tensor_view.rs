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
    /// Configures how the selected slice should fit into the view.
    pub view_fit: crate::blueprint::archetypes::TensorViewFit,

    /// Configures how scalars are mapped to color.
    pub scalar_mapping: crate::blueprint::archetypes::TensorScalarMapping,
}

impl ::re_types_core::SizeBytes for TensorView {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.view_fit.heap_size_bytes() + self.scalar_mapping.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::archetypes::TensorViewFit>::is_pod()
            && <crate::blueprint::archetypes::TensorScalarMapping>::is_pod()
    }
}

impl ::re_types_core::View for TensorView {
    #[inline]
    fn identifier() -> ::re_types_core::SpaceViewClassIdentifier {
        "Tensor".into()
    }
}
