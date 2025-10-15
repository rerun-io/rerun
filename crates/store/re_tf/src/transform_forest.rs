use glam::Affine3A;
use nohash_hasher::IntMap;
use vec1::smallvec_v1::SmallVec1;

use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityPath, EntityTree};
use re_types::ArchetypeName;

use crate::{
    CachedTransformsForTimeline, PoseTransformArchetypeMap, ResolvedPinholeProjection,
    TransformFrameIdHash, TransformResolutionCache, image_view_coordinates,
};

/// Details on how to transform from a source to a target frame.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformInfo {
    /// Root frame this transform belongs to.
    ///
    /// ⚠️ This is the root of the tree this transform belongs to,
    /// not necessarily what the transform transforms into.
    ///
    /// Implementation note:
    /// We could add target and maybe even source to this, but we want to keep this struct small'ish.
    /// On that note, it may be good to split this in the future, as most of the time we're only interested in the
    /// source->target affine transform.
    root: TransformFrameIdHash,

    /// The transform from this frame to the target's space.
    ///
    /// ⚠️ Does not include per instance poses! ⚠️
    /// Include 3D-from-2D / 2D-from-3D pinhole transform if present.
    target_from_source: glam::Affine3A,

    /// List of transforms per instance including poses.
    ///
    /// If no poses are present, this is always the same as [`Self::target_from_source`].
    /// (also implying that in this case there is only a single element).
    /// If there are poses there may be more than one element.
    ///
    /// Does not take into account archetype specific transforms.
    target_from_instances: SmallVec1<[glam::Affine3A; 1]>,

    /// Like [`Self::target_from_instances`] but _on top_ also has archetype specific transforms applied
    /// if there are any present.
    ///
    /// For example, this may have different poses for spheres & boxes.
    target_from_archetype: IntMap<ArchetypeName, SmallVec1<[glam::Affine3A; 1]>>,
}

impl TransformInfo {
    fn new_root(root: TransformFrameIdHash) -> Self {
        Self {
            root,
            target_from_source: glam::Affine3A::IDENTITY,
            target_from_instances: SmallVec1::new(glam::Affine3A::IDENTITY),
            target_from_archetype: Default::default(),
        }
    }

    /// Returns the root frame of the tree this transform belongs to.
    ///
    /// This is **not** necessarily the transform's target frame.
    #[inline]
    pub fn tree_root(&self) -> TransformFrameIdHash {
        self.root
    }

    /// Warns that multiple transforms on an entity are not supported.
    #[inline]
    fn warn_on_per_instance_transform(&self, entity_name: &EntityPath, archetype: ArchetypeName) {
        if self.target_from_instances.len() > 1 {
            re_log::warn_once!(
                "There are multiple poses for entity {entity_name:?}'s transform frame. {archetype:?} supports only one transform per entity. Using the first one."
            );
        }
    }

    /// Returns the first instance transform and warns if there are multiple.
    #[inline]
    pub fn single_transform_required_for_entity(
        &self,
        entity_name: &EntityPath,
        archetype: ArchetypeName,
    ) -> glam::Affine3A {
        self.warn_on_per_instance_transform(entity_name, archetype);

        if let Some(transform) = self.target_from_archetype.get(&archetype) {
            *transform.first()
        } else {
            *self.target_from_instances.first()
        }
    }

    /// Returns the target from instance transforms.
    #[inline]
    pub fn target_from_instances(
        &self,
        archetype: ArchetypeName,
    ) -> &SmallVec1<[glam::Affine3A; 1]> {
        if let Some(transform) = self.target_from_archetype.get(&archetype) {
            transform
        } else {
            &self.target_from_instances
        }
    }

    /// Multiplies all transforms from the left by `target_from_reference`
    ///
    /// Or in other words:
    /// `reference_from_source = self`
    /// `target_from_source = target_from_reference * reference_from_source`
    fn left_multiply(&self, target_from_reference: glam::Affine3A) -> Self {
        let Self {
            root,
            target_from_source: reference_from_source,
            target_from_instances: reference_from_source_instances,
            target_from_archetype: reference_from_source_archetypes,
        } = self;

        let target_from_source = target_from_reference * reference_from_source;
        let target_from_source_instances = left_multiply_smallvec1_of_transforms(
            target_from_reference,
            reference_from_source_instances,
        );
        let target_from_source_archetypes = reference_from_source_archetypes
            .iter()
            .map(|(archetype, transforms)| {
                (
                    *archetype,
                    left_multiply_smallvec1_of_transforms(target_from_reference, transforms),
                )
            })
            .collect();

        Self {
            root: *root,
            target_from_source,
            target_from_instances: target_from_source_instances,
            target_from_archetype: target_from_source_archetypes,
        }
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum TransformFromToError {
    #[error("No transform relationships about the target frame {0:?} are known")]
    UnknownTargetFrame(TransformFrameIdHash),

    #[error("No transform relationships about the source frame {0:?} are known")]
    UnknownSourceFrame(TransformFrameIdHash),

    #[error(
        "There's no path between {target:?} and {src:?}. The target's root is {target_root:?}, the source's root is {source_root:?}"
    )]
    NoPathBetweenFrames {
        target: TransformFrameIdHash,
        src: TransformFrameIdHash, // Can't name this `source` for some strange procmacro reasons
        target_root: TransformFrameIdHash,
        source_root: TransformFrameIdHash,
    },
}

impl TransformFromToError {
    fn no_path_between_target_and_source(target: &TargetInfo, source: &SourceInfo<'_>) -> Self {
        Self::NoPathBetweenFrames {
            target: target.id,
            src: source.id,
            target_root: target.root,
            source_root: source.root,
        }
    }
}

/// Private utility struct for working with a target frame.
struct TargetInfo {
    id: TransformFrameIdHash,
    root: TransformFrameIdHash,
    target_from_root: glam::Affine3A,
}

/// Private utility struct for working with a source frame.
struct SourceInfo<'a> {
    id: TransformFrameIdHash,
    root: TransformFrameIdHash,
    root_from_source: &'a TransformInfo,
}

/// Properties of a pinhole transform tree root.
///
/// Each pinhole forms its own subtree which may be embedded into a 3D space.
/// Everything at and below the pinhole tree root is considered to be 2D,
/// everything above is considered to be 3D.
#[derive(Clone, Debug, PartialEq)]
pub struct PinholeTreeRoot {
    /// The tree root of the parent of this pinhole.
    pub parent_tree_root: TransformFrameIdHash,

    /// Pinhole projection that defines how 2D objects are transformed in this space.
    pub pinhole_projection: ResolvedPinholeProjection,

    /// Transforms the 2D subtree into its parent 3D space.
    pub parent_root_from_pinhole_root: glam::Affine3A,
}

/// Properties of a transform root.
///
/// [`TransformForest`] tries to identify all roots.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformTreeRootInfo {
    /// This is the root of the entity path tree.
    EntityRoot,

    /// The tree root is an entity path with a pinhole transformation,
    /// thus marking a 3D to 2D transition.
    Pinhole(PinholeTreeRoot),
    //
    // TODO(andreas): Something like this will eventually come up:
    //TransformFrameRoot,
}

/// Analyzes & propagates the transform graph of a recording at a given time & timeline.
///
/// Identifies different transform trees present in the recording and computes transforms relative to their roots,
/// such that arbitrary transforms within the tree can be resolved (relatively) quickly.
#[derive(Default, Clone)]
pub struct TransformForest {
    /// All known tree roots.
    roots: IntMap<TransformFrameIdHash, TransformTreeRootInfo>,

    /// All frames reachable from one of the tree roots.
    root_from_frame: IntMap<TransformFrameIdHash, TransformInfo>,
}

impl TransformForest {
    /// Determines transforms for all frames relative.
    /// I.e. the resulting transforms are "reference from scene"
    ///
    /// This means that the entities in `reference_space` get the identity transform and all other
    /// entities are transformed relative to it.
    pub fn new(
        recording: &re_entity_db::EntityDb,
        transform_cache: &TransformResolutionCache,
        time_query: &LatestAtQuery,
    ) -> Self {
        re_tracing::profile_function!();

        let entity_tree = recording.tree();
        let transforms = transform_cache.transforms_for_timeline(time_query.timeline());

        let entity_hierarchy_root = TransformFrameIdHash::entity_path_hierarchy_root();

        let mut tree = Self {
            roots: std::iter::once((entity_hierarchy_root, TransformTreeRootInfo::EntityRoot))
                .collect(),
            root_from_frame: Default::default(),
        };
        tree.entity_tree_gather_transforms_recursive(
            entity_tree,
            time_query,
            // Ignore potential pinhole camera at the root of the view, since it is regarded as being "above" this root.
            // TODO(andreas): Should we warn about that?
            TransformInfo::new_root(entity_hierarchy_root),
            transforms,
        );

        tree
    }
}

impl TransformForest {
    fn entity_tree_gather_transforms_recursive(
        &mut self,
        subtree: &EntityTree,
        query: &LatestAtQuery,
        transform_root_from_parent: TransformInfo,
        transforms_for_timeline: &CachedTransformsForTimeline,
    ) {
        let root = transform_root_from_parent.root;
        let root_from_parent = transform_root_from_parent.target_from_source;

        let transform_frame_id = TransformFrameIdHash::from_entity_path(&subtree.path);
        let previous_transform = self
            .root_from_frame
            .insert(transform_frame_id, transform_root_from_parent);
        debug_assert!(previous_transform.is_none(), "Root was added already"); // TODO(RR-2667): Build out into cycle detection (cycles can't _yet_ happen)

        for child_tree in subtree.children.values() {
            let child_path = &child_tree.path;

            let transforms_at_entity = transforms_at(child_path, query, transforms_for_timeline);

            let root_from_child =
                root_from_parent * transforms_at_entity.parent_from_entity_tree_transform;

            // Did we encounter a pinhole and need to create a new subspace?
            let (root, target_from_source) =
                if let Some(pinhole_projection) = transforms_at_entity.pinhole_projection {
                    let new_root_frame_id = TransformFrameIdHash::from_entity_path(child_path);
                    let new_root_info = TransformTreeRootInfo::Pinhole(PinholeTreeRoot {
                        parent_tree_root: root,
                        pinhole_projection: pinhole_projection.clone(),
                        parent_root_from_pinhole_root: root_from_child,
                    });

                    let previous_root = self.roots.insert(new_root_frame_id, new_root_info);
                    debug_assert!(previous_root.is_none(), "Root was added already"); // TODO(andreas): Build out into cycle detection (cycles can't _yet_ happen)

                    (new_root_frame_id, Affine3A::IDENTITY)
                } else {
                    (root, root_from_child)
                };

            // Collect & compute poses.
            let root_from_instances = compute_root_from_instances(
                target_from_source,
                transforms_at_entity.entity_from_instance_poses,
            );
            let root_from_archetype = compute_root_from_archetype(
                target_from_source,
                transforms_at_entity.entity_from_instance_poses,
            );

            let transform_root_from_child = TransformInfo {
                root,
                target_from_source,
                target_from_instances: root_from_instances,
                target_from_archetype: root_from_archetype,
            };

            self.entity_tree_gather_transforms_recursive(
                child_tree,
                query,
                transform_root_from_child,
                transforms_for_timeline,
            );
        }
    }

    /// Returns the properties of the transform tree root at the given frame.
    ///
    /// If frame is not known as a transform tree root, returns [`None`].
    #[inline]
    pub fn root_info(&self, root_frame: TransformFrameIdHash) -> Option<&TransformTreeRootInfo> {
        self.roots.get(&root_frame)
    }

    /// Returns the properties of the pinhole tree root at the given frame if the frame's root is a pinhole tree root.
    #[inline]
    pub fn pinhole_tree_root_info(
        &self,
        root_frame: TransformFrameIdHash,
    ) -> Option<&PinholeTreeRoot> {
        if let TransformTreeRootInfo::Pinhole(pinhole_tree_root) = self.roots.get(&root_frame)? {
            Some(pinhole_tree_root)
        } else {
            None
        }
    }

    /// Computes the transform from one frame to another if there is a path between them.
    ///
    /// `target`: The frame into which to transform.
    /// `sources`: The frames from which to transform.
    ///
    /// If the target's root & sources are connected with a pinhole camera,
    /// we'll transform it according to the image plane distance.
    ///
    /// Returns an iterator of results, one for each source.
    /// If the target frame is not known at all, returns [`TransformFromToError::UnknownTargetFrame`] for every source.
    pub fn transform_from_to(
        &self,
        target: TransformFrameIdHash,
        sources: impl Iterator<Item = TransformFrameIdHash>,
        lookup_image_plane_distance: &dyn Fn(TransformFrameIdHash) -> f32,
    ) -> impl Iterator<
        Item = (
            TransformFrameIdHash,
            Result<TransformInfo, TransformFromToError>,
        ),
    > {
        // We're looking for a common root between source and target.
        // We start by looking up the target's tree root.

        let Some(root_from_target) = self.root_from_frame.get(&target) else {
            return itertools::Either::Left(sources.map(move |source| {
                (
                    source,
                    Err(TransformFromToError::UnknownTargetFrame(target)),
                )
            }));
        };

        // Invert `root_from_target` to get `target.from_root`.
        let target = {
            let TransformInfo {
                root: target_root,
                target_from_source: root_from_entity,
                // Don't care about instance transforms on the target frame, as they don't tree-propagate.
                target_from_instances: _,
                // Don't care about archetype specific transforms on the target frame, as they don't tree-propagate.
                target_from_archetype: _,
            } = &root_from_target;

            TargetInfo {
                id: target,
                root: *target_root,
                target_from_root: root_from_entity.inverse(),
            }
        };

        // Query type of target's root for later.
        let target_root_info = self.roots.get(&target.root);

        // Local cache for connecting pinhole spaces with their parent 3D space.
        let mut pinhole_tree_connector_cache = IntMap::default();

        itertools::Either::Right(sources.map(move |source| {
            let Some(root_from_source) = self.root_from_frame.get(&source) else {
                return (
                    source,
                    Err(TransformFromToError::UnknownSourceFrame(source)),
                );
            };

            let source = SourceInfo {
                id: source,
                root: root_from_source.root,
                root_from_source,
            };

            // Common case: both source & target share the same root.
            let result = if source.root == target.root {
                if source.root == target.id {
                    // Fast track for source's root being the target.
                    Ok(source.root_from_source.clone())
                } else {
                    // target_from_source = target_from_reference * root_from_source
                    Ok(root_from_source.left_multiply(target.target_from_root))
                }
            }
            // There might be a connection via a pinhole making this 3D in 2D.
            else if let Some(TransformTreeRootInfo::Pinhole(pinhole_tree_root)) = target_root_info
            {
                from_3d_source_to_2d_target(
                    &target,
                    &source,
                    pinhole_tree_root,
                    &mut pinhole_tree_connector_cache,
                )
            }
            // There might be a connection via a pinhole making this 2D in 3D.
            else if let Some(TransformTreeRootInfo::Pinhole(pinhole_tree_root)) =
                self.roots.get(&source.root)
            {
                from_2d_source_to_3d_target(
                    &target,
                    &source,
                    pinhole_tree_root,
                    lookup_image_plane_distance,
                    &mut pinhole_tree_connector_cache,
                )
            }
            // Disconnected, we can't transform into the target space.
            else {
                Err(TransformFromToError::no_path_between_target_and_source(
                    &target, &source,
                ))
            };

            (source.id, result)
        }))
    }
}

fn from_2d_source_to_3d_target(
    target: &TargetInfo,
    source: &SourceInfo<'_>,
    source_pinhole_tree_root: &PinholeTreeRoot,
    lookup_image_plane_distance: &dyn Fn(TransformFrameIdHash) -> f32,
    target_from_image_plane_cache: &mut IntMap<TransformFrameIdHash, glam::Affine3A>,
) -> Result<TransformInfo, TransformFromToError> {
    let PinholeTreeRoot {
        parent_tree_root,
        pinhole_projection,
        parent_root_from_pinhole_root: root_from_pinhole3d,
    } = source_pinhole_tree_root;

    // `root` here is the target's root!
    // We call the source's root `pinhole3d` to distinguish it.
    if *parent_tree_root != target.root {
        return Err(TransformFromToError::no_path_between_target_and_source(
            target, source,
        ));
    }

    // Rename for clarification:
    let image_plane_from_source = source.root_from_source;

    let target_from_image_plane = target_from_image_plane_cache
        .entry(source.root)
        .or_insert_with(|| {
            let pinhole_image_plane_distance = lookup_image_plane_distance(source.root);
            let pinhole3d_from_image_plane =
                pinhole3d_from_image_plane(pinhole_projection, pinhole_image_plane_distance);
            target.target_from_root * root_from_pinhole3d * pinhole3d_from_image_plane
        });

    // target_from_source = target_from_image_plane * image_plane_from_source
    Ok(image_plane_from_source.left_multiply(*target_from_image_plane))
}

fn from_3d_source_to_2d_target(
    target: &TargetInfo,
    source: &SourceInfo<'_>,
    target_pinhole_tree_root: &PinholeTreeRoot,
    target_from_source_root_cache: &mut IntMap<TransformFrameIdHash, glam::Affine3A>,
) -> Result<TransformInfo, TransformFromToError> {
    let PinholeTreeRoot {
        parent_tree_root,
        pinhole_projection,
        parent_root_from_pinhole_root: root_from_pinhole3d,
    } = target_pinhole_tree_root;

    // `root` here is the source's root!
    // We call the target's root `pinhole3d` to distinguish it.
    if *parent_tree_root != source.root {
        return Err(TransformFromToError::no_path_between_target_and_source(
            target, source,
        ));
    }

    // Rename for clarification:
    let target_from_image_plane = target.target_from_root;

    let target_from_root = target_from_source_root_cache
        .entry(source.root)
        .or_insert_with(|| {
            // TODO(#1025):
            // There's no meaningful image plane distance for 3D->2D views.
            let pinhole_image_plane_distance = 500.0;
            // Currently our 2D views require us to invert the `pinhole2d_image_plane_from_pinhole3d` matrix.
            // This builds a relationship between the 2D plane and the 3D world, when actually the 2D plane
            // should have infinite depth!
            // The inverse of this matrix *is* working for this, but quickly runs into precision issues.
            // See also `ui_2d.rs#setup_target_config`

            let pinhole3d_from_image_plane =
                pinhole3d_from_image_plane(pinhole_projection, pinhole_image_plane_distance);
            let image_plane_from_pinhole3d = pinhole3d_from_image_plane.inverse();
            let pinhole3d_from_root = root_from_pinhole3d.inverse();
            target_from_image_plane * image_plane_from_pinhole3d * pinhole3d_from_root
        });

    // target_from_source = target_from_root * root_from_source
    Ok(source.root_from_source.left_multiply(*target_from_root))
}

fn left_multiply_smallvec1_of_transforms(
    target_from_reference: glam::Affine3A,
    reference_from_source: &SmallVec1<[glam::Affine3A; 1]>,
) -> SmallVec1<[glam::Affine3A; 1]> {
    // Easiest to deal with SmallVec1 in-place.
    let mut target_from_source = reference_from_source.clone();
    for transform in &mut target_from_source {
        *transform = target_from_reference * *transform;
    }
    target_from_source
}

fn compute_root_from_poses(
    root_from_entity: glam::Affine3A,
    instance_from_poses: &[glam::Affine3A],
) -> SmallVec1<[glam::Affine3A; 1]> {
    let Ok(mut reference_from_poses) =
        SmallVec1::<[glam::Affine3A; 1]>::try_from_slice(instance_from_poses)
    else {
        return SmallVec1::new(root_from_entity);
    };

    // Until now `reference_from_poses` is actually `reference_from_entity`.
    for reference_from_pose in &mut reference_from_poses {
        let entity_from_pose = *reference_from_pose;
        *reference_from_pose = root_from_entity * entity_from_pose;
    }
    reference_from_poses
}

fn compute_root_from_instances(
    reference_from_entity: glam::Affine3A,
    pose_transforms: Option<&PoseTransformArchetypeMap>,
) -> SmallVec1<[glam::Affine3A; 1]> {
    compute_root_from_poses(
        reference_from_entity,
        pose_transforms.map_or(&[], |poses| &poses.instance_from_poses),
    )
}

fn compute_root_from_archetype(
    reference_from_entity: glam::Affine3A,
    entity_from_instance_poses: Option<&PoseTransformArchetypeMap>,
) -> IntMap<ArchetypeName, SmallVec1<[glam::Affine3A; 1]>> {
    entity_from_instance_poses
        .map(|poses| {
            poses
                .instance_from_archetype_poses_per_archetype
                .iter()
                .map(|(archetype, poses)| {
                    (
                        *archetype,
                        compute_root_from_poses(reference_from_entity, poses),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn pinhole3d_from_image_plane(
    resolved_pinhole_projection: &ResolvedPinholeProjection,
    pinhole_image_plane_distance: f32,
) -> glam::Affine3A {
    let ResolvedPinholeProjection {
        image_from_camera,
        resolution: _,
        view_coordinates,
    } = resolved_pinhole_projection;

    // Everything under a pinhole camera is a 2D projection, thus doesn't actually have a proper 3D representation.
    // Our visualization interprets this as looking at a 2D image plane from a single point (the pinhole).

    // Center the image plane and move it along z, scaling the further the image plane is.
    let focal_length = image_from_camera.focal_length_in_pixels();
    let focal_length = glam::vec2(focal_length.x(), focal_length.y());
    let scale = pinhole_image_plane_distance / focal_length;
    let translation =
        (-image_from_camera.principal_point() * scale).extend(pinhole_image_plane_distance);

    let image_plane3d_from_2d_content = glam::Affine3A::from_translation(translation)
            // We want to preserve any depth that might be on the pinhole image.
            // Use harmonic mean of x/y scale for those.
            * glam::Affine3A::from_scale(
                scale.extend(2.0 / (1.0 / scale.x + 1.0 / scale.y)),
            );

    // Our interpretation of the pinhole camera implies that the axis semantics, i.e. ViewCoordinates,
    // determine how the image plane is oriented.
    // (see also `CamerasPart` where the frustum lines are set up)
    let obj_from_image_plane3d = view_coordinates.from_other(&image_view_coordinates());

    glam::Affine3A::from_mat3(obj_from_image_plane3d) * image_plane3d_from_2d_content

    // Above calculation is nice for a certain kind of visualizing a projected image plane,
    // but the image plane distance is arbitrary and there might be other, better visualizations!
}

/// Resolved transforms at an entity.
#[derive(Default)]
struct TransformsAtEntity<'a> {
    parent_from_entity_tree_transform: glam::Affine3A,
    entity_from_instance_poses: Option<&'a PoseTransformArchetypeMap>,
    pinhole_projection: Option<&'a ResolvedPinholeProjection>,
}

fn transforms_at<'a>(
    entity_path: &EntityPath,
    query: &LatestAtQuery,
    transforms_for_timeline: &'a CachedTransformsForTimeline,
) -> TransformsAtEntity<'a> {
    // This is called very frequently, don't put a profile scope here.

    let Some(entity_transforms) = transforms_for_timeline.entity_transforms(entity_path) else {
        return TransformsAtEntity::default();
    };

    let parent_from_entity_tree_transform = entity_transforms.latest_at_tree_transform(query);
    let entity_from_instance_poses = entity_transforms.latest_at_instance_poses_all(query);
    let pinhole_projection = entity_transforms.latest_at_pinhole(query);

    TransformsAtEntity {
        parent_from_entity_tree_transform,
        entity_from_instance_poses,
        pinhole_projection,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::Chunk;
    use re_entity_db::EntityDb;
    use re_log_types::{StoreInfo, TimePoint, TimelineName};
    use re_types::{Archetype as _, RowId, archetypes, components::TransformFrameId};

    use super::*;

    fn test_pinhole() -> archetypes::Pinhole {
        archetypes::Pinhole::from_focal_length_and_resolution([1.0, 2.0], [100.0, 200.0])
    }

    /// A test scene that relies exclusively on the entity hierarchy.
    ///
    /// We're using relatively basic transforms here as we assume that resolving transforms have been tested on [`TransformResolutionCache`] already.
    /// Similarly, since [`TransformForest`] does not yet maintain anything over time, we're using static timing instead.
    /// TODO(RR-2510): add another scene (or extension) where we override transforms on select entities
    /// TODO(RR-2511): add a scene with frame relationships
    fn entity_hierarchy_test_scene() -> EntityDb {
        let mut entity_db = EntityDb::new(StoreInfo::testing().store_id);
        entity_db
            .add_chunk(&Arc::new(
                Chunk::builder(EntityPath::from("top"))
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &archetypes::Transform3D::from_translation([1.0, 0.0, 0.0]),
                    )
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        // Add some instance transforms - we need to make sure they don't propagate.
                        &archetypes::InstancePoses3D::new()
                            .with_translations([[10.0, 0.0, 0.0], [20.0, 0.0, 0.0]]),
                    )
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        // Add some boxes - its centers are handled treated like special instance transforms.
                        &archetypes::Boxes3D::update_fields()
                            .with_centers([[0.0, 10.0, 0.0], [0.0, 20.0, 0.0]]),
                    )
                    .build()
                    .unwrap(),
            ))
            .unwrap();
        entity_db
            .add_chunk(&Arc::new(
                Chunk::builder(EntityPath::from("top/pinhole"))
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &archetypes::Transform3D::from_translation([0.0, 1.0, 0.0]),
                    )
                    .with_archetype(RowId::new(), TimePoint::STATIC, &test_pinhole())
                    .build()
                    .unwrap(),
            ))
            .unwrap();

        entity_db
            .add_chunk(&Arc::new(
                Chunk::builder(EntityPath::from("top/pinhole/child2d"))
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &archetypes::Transform3D::from_translation([2.0, 0.0, 0.0]),
                    )
                    .build()
                    .unwrap(),
            ))
            .unwrap();
        entity_db
            .add_chunk(&Arc::new(
                Chunk::builder(EntityPath::from("top/child3d"))
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &archetypes::Transform3D::from_translation([0.0, 0.0, 1.0]),
                    )
                    .build()
                    .unwrap(),
            ))
            .unwrap();

        entity_db
    }

    fn pretty_print_transform_frame_ids_in<T: std::fmt::Debug>(
        obj: T,
        frames: &[TransformFrameId],
    ) -> String {
        let mut result = format!("{obj:#?}");
        for frame in frames {
            result = result.replace(
                &format!("{:#?}", TransformFrameIdHash::new(frame)),
                &format!("{frame}"),
            );
        }
        result
    }

    #[test]
    fn test_simple_entity_hierarchy() {
        let test_scene = entity_hierarchy_test_scene();
        let mut transform_cache = TransformResolutionCache::default();
        transform_cache.add_chunks(
            &test_scene,
            test_scene.storage_engine().store().iter_chunks(),
        );

        let query = LatestAtQuery::latest(TimelineName::log_tick());
        let transform_forest = TransformForest::new(&test_scene, &transform_cache, &query);

        let all_entity_paths = test_scene
            .entity_paths()
            .into_iter()
            .cloned()
            .chain([EntityPath::root(), EntityPath::from("top/nonexistent")])
            .collect::<Vec<_>>();
        let all_transform_frame_ids = all_entity_paths
            .iter()
            .map(TransformFrameId::from_entity_path)
            .collect::<Vec<_>>();

        // Check that we get the expected roots.
        {
            assert_eq!(
                transform_forest.root_info(TransformFrameIdHash::entity_path_hierarchy_root()),
                Some(&TransformTreeRootInfo::EntityRoot)
            );
            // Pinhole roots are a bit more complex. Let's use `insta` to verify.
            insta::assert_snapshot!(
                "simple_entity_hierarchy__root_info_pinhole",
                pretty_print_transform_frame_ids_in(
                    transform_forest.root_info(TransformFrameIdHash::from_entity_path(
                        &EntityPath::from("top/pinhole")
                    )),
                    &all_transform_frame_ids
                )
            );
            // .. but it's hard to reason about the parent root id, so let's verify that just to be sure.
            assert_eq!(
                transform_forest
                    .pinhole_tree_root_info(TransformFrameIdHash::from_entity_path(
                        &EntityPath::from("top/pinhole")
                    ))
                    .unwrap()
                    .parent_tree_root,
                TransformFrameIdHash::entity_path_hierarchy_root()
            );
            assert_eq!(transform_forest.roots.len(), 2);
        }

        // Perform some tree queries.
        let target_paths = [
            EntityPath::root(),
            EntityPath::from("top"),
            EntityPath::from("top/pinhole"),
            EntityPath::from("top/nonexistent"),
            EntityPath::from("top/pinhole/child2d"),
        ];
        let source_paths = [
            EntityPath::root(),
            EntityPath::from("top"),
            EntityPath::from("top/pinhole"),
            EntityPath::from("top/child3d"),
            EntityPath::from("top/nonexistent"),
            EntityPath::from("top/pinhole/child2d"),
        ];

        for target in &target_paths {
            let name = if target == &EntityPath::root() {
                "_root".to_owned()
            } else {
                target.to_string().replace('/', "_")
            };

            let target_frame = TransformFrameIdHash::from_entity_path(target);
            let result = transform_forest
                .transform_from_to(
                    target_frame,
                    source_paths
                        .iter()
                        .map(TransformFrameIdHash::from_entity_path),
                    &|_| 1.0,
                )
                .collect::<Vec<_>>();

            // If the target exists, it should have an identity transform.
            // (this is covered by the snapshot below as well, but its a basic sanity check I wanted to call out)
            let target_result = result.iter().find(|(key, _)| *key == target_frame).unwrap();
            if let Ok(target_result) = &target_result.1 {
                assert!(target_result.target_from_source == glam::Affine3A::IDENTITY);
            } else {
                assert_eq!(
                    target_result.1,
                    Err(TransformFromToError::UnknownTargetFrame(target_frame))
                );
            }

            insta::assert_snapshot!(
                format!("simple_entity_hierarchy__transform_from_to_{}", name),
                pretty_print_transform_frame_ids_in(&result, &all_transform_frame_ids)
            );
        }
    }

    /// Regression test for <https://github.com/rerun-io/rerun/issues/11496>
    ///
    /// This is redundant with `test_simple_entity_hierarchy` but it's good to call this out separately since
    /// it might easily be missed in the snapshot update.
    #[test]
    fn test_instance_transforms_at_target_frame() {
        let mut entity_db = EntityDb::new(StoreInfo::testing().store_id);
        entity_db
            .add_chunk(&Arc::new(
                Chunk::builder(EntityPath::from("box"))
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &archetypes::Transform3D::from_translation([1.0, 0.0, 0.0]),
                    )
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &archetypes::InstancePoses3D::new()
                            .with_translations([[0.0, 10.0, 0.0], [0.0, 20.0, 0.0]]),
                    )
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &archetypes::Boxes3D::update_fields()
                            .with_centers([[0.0, 0.0, 100.0], [0.0, 0.0, 200.0]]),
                    )
                    .build()
                    .unwrap(),
            ))
            .unwrap();

        let mut transform_cache = TransformResolutionCache::default();
        transform_cache.add_chunks(&entity_db, entity_db.storage_engine().store().iter_chunks());

        let query = LatestAtQuery::latest(TimelineName::log_tick());
        let transform_forest = TransformForest::new(&entity_db, &transform_cache, &query);

        let target = TransformFrameIdHash::from_entity_path(&EntityPath::from("box"));
        let sources = [TransformFrameIdHash::from_entity_path(&EntityPath::from(
            "box",
        ))];

        let result = transform_forest
            .transform_from_to(target, sources.iter().copied(), &|_| 1.0)
            .collect::<Vec<_>>();

        assert_eq!(result.len(), 1);
        let (source, result) = &result[0];
        assert_eq!(source, &target);
        let info = result.as_ref().unwrap();
        assert_eq!(
            info.root,
            TransformFrameIdHash::entity_path_hierarchy_root()
        );

        // It *is* the target, so identity for this!
        assert_eq!(info.target_from_source, glam::Affine3A::IDENTITY);

        // Instance transforms still apply.
        assert_eq!(
            info.target_from_instances,
            SmallVec1::<[glam::Affine3A; 1]>::try_from_slice(&[
                glam::Affine3A::from_translation(glam::vec3(0.0, 10.0, 0.0)),
                glam::Affine3A::from_translation(glam::vec3(0.0, 20.0, 0.0))
            ])
            .unwrap()
        );

        // Archetype specific transforms still apply _on top_ of the instance transforms.
        assert_eq!(
            info.target_from_archetype,
            std::iter::once((
                archetypes::Boxes3D::name(),
                SmallVec1::<[glam::Affine3A; 1]>::try_from_slice(&[
                    glam::Affine3A::from_translation(glam::vec3(0.0, 10.0, 100.0)),
                    glam::Affine3A::from_translation(glam::vec3(0.0, 20.0, 200.0))
                ])
                .unwrap(),
            ))
            .collect()
        );
    }
}
