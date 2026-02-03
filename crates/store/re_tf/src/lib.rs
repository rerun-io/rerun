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
//! Transform frames are identified by a string identifier, see [`re_sdk_types::components::TransformFrameId`].
//!
//! Spatial transforms may change over time to model movement of an object and its parts.
//! While typically fairly fixed, the topology of the transform graph may change over time.
//!
//! A valid transform graph is expected to form a forest, i.e. one or more trees.
//!
//! #### Entity relationship & built-in transform frames
//!
//! Every entity is associated with a transform frame.
//! The transform frame can be set with [`re_sdk_types::archetypes::CoordinateFrame`].
//!
//! However, by default, it points to an implicit, entity-derived transform frame.
//! The name of the implicit transform frames is the entity path, prefixed with `tf#`, e.g. `tf#/world/robot/arm`.
//!
//! Entity derived transform frames automatically have an identity transform relationship
//! to their respective parent (unless overwritten by e.g. [`re_sdk_types::archetypes::Transform3D`]).
//!
//! Example:
//! Given an entity hierarchy:
//! ```text
//! world
//! |-- robot
//! |   |-- left_arm
//! |   |-- right_arm
//! ```
//! Without setting any transform frames, this means we have a identity connected tree
//! shown to the left that is associated with individual entities on the right:
//! ```text
//!                ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐                                    ┌──────────────────┐
//!                          tf#world          ◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│      world       │
//!                └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘                                    └──────────────────┘
//!                              │                                                            │
//!                              │                                                            │
//!                              │                                                            │
//!                              │                                                            │
//!                ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐                                    ┌──────────────────┐
//!                    tf#world/robot           ◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │   world/robot    │
//!                └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘                                    └──────────────────┘
//!                              Λ                                                            Λ
//!                             ╱ ╲                                                          ╱ ╲
//!                            ╱   ╲                                                        ╱   ╲
//!                  ╱────────╱     ╲────────╲                                      ╱──────╱     ╲──────╲
//!                 ╱                         ╲                                    ╱                     ╲
//!                ╱                           ╲                                  ╱                       ╲
//!               ╱                             ╲                                ╱                         ╲
//! ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐ ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐        ┌──────────────────┐      ┌──────────────────┐
//!      tf#world/robot/left           tf#world/robot/right            │ world/robot/left │      │world/robot/right │
//! └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘ └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘        └──────────────────┘      └──────────────────┘
//!               ▲                             ▲                                │                         │
//!               │
//!                ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘                         │
//!                                              ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘
//! ```
//!
//! You can interact with these implicitly generated frames like with any other transform frame!
//! For example, let's say we log a manual transform relationship between two new frames called `robot_frame`
//! and `left_frame`, associate them with `world/robot` and `world/robot/left` respectively.
//! That would create two unconnected trees, but this can be handled by specifying another
//! relationship from `robot_frame` to `tf#world/robot`, leading to this setup:
//! ```text
//!                                     ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐                                   ┌──────────────────┐
//!                                               tf#world         ◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│      world       │
//!                                     └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘                                   └──────────────────┘
//!                                                   │                                                           │
//!         ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┼ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐                                 │
//!                                                   │                                                           │
//!         ▼                                         │                         │                                 │
//! ┌ ─ ─ ─ ─ ─ ─ ─ ┐                   ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐                                   ┌──────────────────┐
//!    robot_frame   ───────────────────       tf#world/robot                   └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │   world/robot    │
//! └ ─ ─ ─ ─ ─ ─ ─ ┘                   └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘                                   └──────────────────┘
//!         │                                         Λ                                                           Λ
//!         │                                        ╱ ╲                                                         ╱ ╲
//!         │                                       ╱   ╲                                                       ╱   ╲
//!         │                             ╱────────╱     ╲────────╲                                     ╱──────╱     ╲──────╲
//!         │                            ╱                         ╲                                   ╱                     ╲
//!         │                           ╱                           ╲                                 ╱                       ╲
//!         │                          ╱                             ╲                               ╱                         ╲
//! ┌ ─ ─ ─ ─ ─ ─ ─ ┐    ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐ ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐       ┌──────────────────┐      ┌──────────────────┐
//!    left_frame             tf#world/robot/left           tf#world/robot/right           │ world/robot/left │      │world/robot/right │
//! └ ─ ─ ─ ─ ─ ─ ─ ┘    └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘ └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘       └──────────────────┘      └──────────────────┘
//!         ▲                                                        ▲                               │                         │
//!         │                                                        │
//!          ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘                         │
//!                                                                  └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘
//! ```
//!
//! ### Instance poses
//!
//! Instance poses (or just poses) define a transform on top of a frame which are independent of the
//! frame graph structure and are not propagated through the transform tree.
//!
//! Conceptually, each pose transform forms a relationship from a frame-instance to its hosting frame.
//!
//! For more details see [`re_sdk_types::archetypes::InstancePoses3D`].
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
//! This is a fairly complex workload since combining everything in [`re_sdk_types::archetypes::Transform3D`]
//! and other transform archetypes into a single affine transform is not trivial and needs to be done
//! in the correct order.
//!
//! The [`TransformResolutionCache`] makes sure that (latest-at) lookups of affine transforms
//! can be performed efficiently while heeding the query rules established by [`re_entity_db::external::re_query`].
//!
//!
//! ### [`TransformFrameIdHash`]
//!
//! Throughout this crate, we almost always use pre-hashed [`re_sdk_types::components::TransformFrameId`]s, i.e. [`TransformFrameIdHash`].
//! Hashes are assumed to be collision free, allowing use as keys in maps.
//!
//! For performance & convenience, there's a 1:1 mapping from [`re_log_types::EntityPathHash`] to [`TransformFrameIdHash`]
//! for referring to built-in transform frames.
//!

mod frame_id_registry;
mod transform_aspect;
mod transform_forest;
mod transform_queries;
mod transform_resolution_cache;

pub mod convert;

// Re-export the transform frame id types from re_sdk_types.
pub use re_sdk_types::TransformFrameIdHash;
pub use re_sdk_types::components::TransformFrameId;

pub use self::frame_id_registry::FrameIdRegistry;
pub use self::transform_forest::{
    PinholeTreeRoot, TransformForest, TransformFromToError, TreeTransform,
};
pub use self::transform_queries::{
    query_view_coordinates, query_view_coordinates_at_closest_ancestor,
};
pub use self::transform_resolution_cache::{
    CachedTransformsForTimeline, ResolvedPinholeProjection, TransformResolutionCache,
};

/// Returns the view coordinates used for 2D (image) views.
///
/// TODO(#1387): Image coordinate space should be configurable.
pub fn image_view_coordinates() -> re_sdk_types::components::ViewCoordinates {
    re_sdk_types::archetypes::Pinhole::DEFAULT_CAMERA_XYZ
}
