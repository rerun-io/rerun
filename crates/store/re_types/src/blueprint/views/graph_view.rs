// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/rust/api.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/views/graph.fbs".

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

/// **View**: A graph view to display time-variying, directed or undirected graph visualization.
#[derive(Clone, Debug)]
pub struct GraphView {
    /// Everything within these bounds is guaranteed to be visible.
    ///
    /// Somethings outside of these bounds may also be visible due to letterboxing.
    pub visual_bounds: crate::blueprint::archetypes::VisualBounds2D,

    /// A link force between nodes in the graph.
    pub force_link: crate::blueprint::archetypes::ForceLink,

    /// TODO
    pub force_many_body: crate::blueprint::archetypes::ForceManyBody,

    /// TODO
    pub force_position: crate::blueprint::archetypes::ForcePosition,

    /// TODO
    pub force_collision_radius: crate::blueprint::archetypes::ForceCollisionRadius,

    /// TODO
    pub force_center: crate::blueprint::archetypes::ForceCenter,
}

impl ::re_types_core::SizeBytes for GraphView {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.visual_bounds.heap_size_bytes()
            + self.force_link.heap_size_bytes()
            + self.force_many_body.heap_size_bytes()
            + self.force_position.heap_size_bytes()
            + self.force_collision_radius.heap_size_bytes()
            + self.force_center.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        <crate::blueprint::archetypes::VisualBounds2D>::is_pod()
            && <crate::blueprint::archetypes::ForceLink>::is_pod()
            && <crate::blueprint::archetypes::ForceManyBody>::is_pod()
            && <crate::blueprint::archetypes::ForcePosition>::is_pod()
            && <crate::blueprint::archetypes::ForceCollisionRadius>::is_pod()
            && <crate::blueprint::archetypes::ForceCenter>::is_pod()
    }
}

impl ::re_types_core::View for GraphView {
    #[inline]
    fn identifier() -> ::re_types_core::SpaceViewClassIdentifier {
        "Graph".into()
    }
}
