//! # Rerun spatial transform processing
//!
//! ## Concepts
//!
//! ### Transform frames
//!
//! A transform frame is a geometric reference frame that may be connected via an affine transform to another reference frame.
//! For instance, the transform frame of a robot body may be connected to the transform frame of the robot's arm
//! via a translation & rotation.
//!
//! Transform frames are identified by a string identifier, see [`re_types::components::TransformFrameId`].
//!
//! Spatial transforms may change over time to model movement of an object and its parts.
//! While typically fairly fixed, the topology of the transform graph may change over time.
//!
//! A valid transform graph is expected to form a forest, i.e. one or more trees.
//!
//! TODO(RR-2511): We don't actually have custom frame relationships yet.
//!
//! #### Entity relationship & built-in transform frames
//!
//! TODO(RR-2487): Most things in this paragraph are planned but not yet implemented.
//!
//! Every entity is associated with a transform frame.
//! The transform frame can be set with the `CoordinateFrame` archetype.
//! TODO(RR-2486): Link to respective archetype.
//!
//! However, by default, it points to an implicit, entity-derived transform frame.
//! The name of the implicit transform frames is the entity path, prefixed with `rerun_tf#`, e.g. `rerun_tf#/world/robot/arm`.
//!
//! Entity derived transform frames automatically have an identity transform relationship
//! to their parent & child's transform frames (_not_ their derived transform frames, but whatever their frames are set to!).
//! This means that whenever an entity changes its transform frame,
//! we automatically create new identity relationships.
//!
//! Example:
//! Given an entity hierarchy:
//! ```text
//! world
//! |-- robot
//! |   |-- left_arm
//! |   |-- right_arm
//! ```
//! Without setting any transform frames, the implicit transform frames would be:
//! ```text
//! world <=> rerun_tf#/world
//! world/robot <=> rerun_tf#/world/robot
//! world/robot/left_arm <=> rerun_tf#/world/robot/left_arm
//! world/robot/right_arm <=> rerun_tf#/world/robot/right_arm
//! ```
//! All of these transform frames are automatically connected via an identity transform.
//! Now we change the transform frame `world/robot` & `world/robot/left_arm`:
//! ```text
//! world <=> rerun_tf#/world
//! world/robot <=> robot_frame
//! world/robot/left_arm <=> robot_left_arm
//! world/robot/right_arm <=> rerun_tf#/world/robot/right_arm
//! ```
//! Then, there's automatically new identity relationships created between:
//! ```text
//! robot_frame -> rerun_tf#/world
//! rerun_tf#/world/robot/right_arm -> robot_frame
//! ```
//!
//!
//! ### Instance poses
//!
//! Instance poses (or just poses) define a transform on top of a frame which are independent of the
//! frame graph structure and are not propagated through the transform tree.
//!
//! Conceptually, each pose transform forms a relationship from a frame-instance to its hosting frame.
//!
//! For more details see [`re_types::archetypes::InstancePoses3D`].
//!
//!
//! ## Implementation
//!
//!
//! ### [`TransformForest`]
//!
//! Analyzes & propagates the transform graph such that querying relationships from
//! any source frame to any target frames for a given time can be done efficiently.
//!
//! ### [`TransformResolutionCache`]
//!
//! Resolves transform relationships over time to standardized affine (mat3x3 + translation) transforms.
//! This is a fairly complex workload since combining everything in [`re_types::archetypes::Transform3D`]
//! and other transform archetypes into a single affine transform is not trivial and needs to be done
//! in the correct order.
//!
//! The [`TransformResolutionCache`] makes sure that (latest-at) lookups of affine transforms
//! can be performed efficiently while heeding the query rules established by [`re_entity_db::external::re_query`].
//!
//!
//! ### [`TransformFrameIdHash`]
//!
//! Throughout this crate, we almost always use pre-hashed [`re_types::components::TransformFrameId`]s, i.e. [`TransformFrameIdHash`].
//! Hashes are assumed to be collision free, allowing use as keys in maps.
//!
//! For performance & convenience, there's a 1:1 mapping from [`re_log_types::EntityPathHash`] to [`TransformFrameIdHash`]
//! for referring to built-in transform frames.
//!

mod component_type_info;
mod transform_forest;
mod transform_frame_id_hash;
mod transform_resolution_cache;

pub use transform_forest::{PinholeTreeRoot, TransformForest, TransformFromToError, TransformInfo};
pub use transform_frame_id_hash::TransformFrameIdHash;
pub use transform_resolution_cache::{
    CachedTransformsForTimeline, PoseTransformArchetypeMap, ResolvedPinholeProjection,
    TransformResolutionCache, query_view_coordinates, query_view_coordinates_at_closest_ancestor,
};

/// Returns the view coordinates used for 2D (image) views.
///
/// TODO(#1387): Image coordinate space should be configurable.
pub fn image_view_coordinates() -> re_types::components::ViewCoordinates {
    re_types::archetypes::Pinhole::DEFAULT_CAMERA_XYZ
}
