use nohash_hasher::{IntMap, IntSet};
use vec1::smallvec_v1::SmallVec1;

use crate::frame_id_registry::FrameIdRegistry;
use crate::transform_resolution_cache::ParentFromChildTransform;
use crate::{
    CachedTransformsForTimeline, ResolvedPinholeProjection, TransformFrameIdHash,
    TransformResolutionCache, image_view_coordinates,
};
use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityDb, EntityPath};
use re_types::ArchetypeName;

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
    target_from_source: glam::DAffine3,

    /// List of transforms per instance including poses.
    ///
    /// If no poses are present, this is always the same as [`Self::target_from_source`].
    /// (also implying that in this case there is only a single element).
    /// If there are poses, there may be more than one element.
    target_from_instances: SmallVec1<[glam::DAffine3; 1]>,
}

impl TransformInfo {
    fn new_root(root: TransformFrameIdHash) -> Self {
        Self {
            root,
            target_from_source: glam::DAffine3::IDENTITY,
            target_from_instances: SmallVec1::new(glam::DAffine3::IDENTITY),
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
    ) -> glam::DAffine3 {
        self.warn_on_per_instance_transform(entity_name, archetype);
        *self.target_from_instances.first()
    }

    /// Returns the target from instance transforms.
    #[inline]
    pub fn target_from_instances(&self) -> &SmallVec1<[glam::DAffine3; 1]> {
        &self.target_from_instances
    }

    /// Multiplies all transforms from the left by `target_from_reference`
    ///
    /// Or in other words:
    /// `reference_from_source = self`
    /// `target_from_source = target_from_reference * reference_from_source`
    fn left_multiply(&self, target_from_reference: glam::DAffine3) -> Self {
        let Self {
            root,
            target_from_source: reference_from_source,
            target_from_instances: reference_from_source_instances,
        } = self;

        let target_from_source = target_from_reference * reference_from_source;
        let target_from_source_instances = left_multiply_smallvec1_of_transforms(
            target_from_reference,
            reference_from_source_instances,
        );

        Self {
            root: *root,
            target_from_source,
            target_from_instances: target_from_source_instances,
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
    target_from_root: glam::DAffine3,
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
    pub parent_root_from_pinhole_root: glam::DAffine3,
}

/// Properties of a transform root.
///
/// [`TransformForest`] tries to identify all roots.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformTreeRootInfo {
    /// Regular root without any extra meta information.
    TransformFrameRoot,

    /// The tree root is an entity path with a pinhole transformation,
    /// thus marking a 3D to 2D transition.
    Pinhole(PinholeTreeRoot),
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
    ///
    /// Roots are also contained, targeting themselves with identity.
    /// This simplifies lookups.
    root_from_frame: IntMap<TransformFrameIdHash, TransformInfo>,
    //
    // TODO(RR-2667): Store errors that occur during the graph walk
}

impl TransformForest {
    /// Computes a forest (several trees) of all transforms for a given point in time.
    pub fn new(
        entity_db: &EntityDb,
        transform_cache: &TransformResolutionCache,
        query: &LatestAtQuery,
    ) -> Self {
        re_tracing::profile_function!();

        // Algorithm overview:
        //
        // We're using a dynamic programming approach that minimizes queries into the transform cache.
        //
        // 1) `walk_towards_parent`:
        // For a given unprocessed frame, we walk towards their parents until there's either no more connection or we hit a frame
        // that we already visited. As we walk, we collect all the encountered transforms into a "transform stack" and mark visited frames as processed.
        // 2) `add_stack_of_transforms`:
        // Then, apply this transform stack to the existing datastructures.
        // First, we figure out which root this stack belongs to, then we walk the stack backwards,
        // computing the respective transforms to the root as we go.
        //
        // Repeat steps 1) & 2) until we've processed all frames.

        let transforms = transform_cache.transforms_for_timeline(query.timeline());
        let mut unprocessed_frames: IntSet<_> = transform_cache
            .frame_id_registry()
            .iter_frame_id_hashes()
            .collect();
        let mut transform_stack = Vec::new(); // Keep pushing & draining from the same vector as a simple performance optimization.

        let mut forest = Self::default();

        // Pop an arbitrary source frame from the list of unprocessed frames.
        while let Some(current_frame) = unprocessed_frames.iter().next().copied() {
            // Walk as long as we can until we hit something we already processed or end up in a dead end.
            walk_towards_parent(
                entity_db,
                query,
                current_frame,
                transform_cache.frame_id_registry(),
                transforms,
                &mut unprocessed_frames,
                &mut transform_stack,
            );

            // Process the stack we accumulated.
            debug_assert!(
                !transform_stack.is_empty(),
                "There should be at least one element in the transform stack since we know we had at least one unprocessed element to start with."
            );
            forest.add_stack_of_transforms(transform_cache, &mut transform_stack);
            debug_assert!(
                transform_stack.is_empty(),
                "Expected add_stack_of_transforms to consume an entire transform stack."
            );
        }

        forest
    }

    /// Adds a stack of transforms produced by [`walk_towards_parent`] to the forest.
    ///
    /// Each stack in the transform is a parent item of the item before it.
    fn add_stack_of_transforms(
        &mut self,
        cache: &TransformResolutionCache,
        transform_stack: &mut Vec<ParentChildTransforms>,
    ) {
        re_tracing::profile_function!();

        let Some(top_of_stack) = transform_stack.last() else {
            // Should never happen in regular operation.
            return;
        };

        // Figure out the root frame for this entire stack.
        let (mut root_frame, mut root_from_target) = if let Some(parent_from_child) =
            top_of_stack.parent_from_child.as_ref()
        {
            // We have a connection further up the stack. That means we must have stopped because we already know that target!
            if let Some(root_from_frame) = self.root_from_frame.get(&parent_from_child.parent) {
                // Yes, we can short-circuit to a known root!
                debug_assert!(self.roots.contains_key(&root_from_frame.root));
                (root_from_frame.root, root_from_frame.target_from_source)
            } else {
                // We didn't know the target. Must mean that the target is a new root!
                let previous_root = self.roots.insert(
                    parent_from_child.parent,
                    // There's apparently no information about this root, so it can't be a pinhole!
                    TransformTreeRootInfo::TransformFrameRoot,
                );
                debug_assert!(previous_root.is_none(), "Root was added already"); // TODO(RR-2667): Build out into cycle detection

                // That parent apparently won't show up in any transform stack (we didn't walk there because there was no information about it!)
                // So if we don't add this root now to our `root_from_frame` map, we'd never fill out the required self-reference!
                self.root_from_frame.insert(
                    parent_from_child.parent,
                    TransformInfo::new_root(parent_from_child.parent),
                );

                (parent_from_child.parent, glam::DAffine3::IDENTITY)
            }
        } else {
            // We're not pointing at a new root. So we ourselves must be a root!
            let previous_root = if let Some(pinhole_projection) = &top_of_stack.pinhole_projection {
                // We're a (lonely) pinhole with no 3D parent.
                // Usually pinholes are embedded into a 3D space, but this one doesn't have any more parents,
                // meaning that everything in this entire tree is 2D.
                let new_root_info = TransformTreeRootInfo::Pinhole(PinholeTreeRoot {
                    parent_tree_root: top_of_stack.child_frame,
                    pinhole_projection: pinhole_projection.clone(),
                    parent_root_from_pinhole_root: glam::DAffine3::IDENTITY,
                });
                self.roots.insert(top_of_stack.child_frame, new_root_info)
            } else {
                self.roots.insert(
                    top_of_stack.child_frame,
                    TransformTreeRootInfo::TransformFrameRoot,
                )
            };
            debug_assert!(previous_root.is_none(), "Root was added already"); // TODO(RR-2667): Build out into cycle detection

            (top_of_stack.child_frame, glam::DAffine3::IDENTITY)
        };

        // Walk the stack backwards, collecting transforms as we go.
        while let Some(transforms) = transform_stack.pop() {
            let mut root_from_current_frame = root_from_target
                * transforms
                    .parent_from_child
                    // Identity here means we're self-referencing. That's fine since we want roots to refer to themselves in our look-up table.
                    .map_or(glam::DAffine3::IDENTITY, |target_from_source| {
                        target_from_source.transform
                    });

            // Did we encounter a pinhole and need to create a new subspace?
            if let Some(pinhole_projection) = transforms.pinhole_projection {
                let new_root_info = TransformTreeRootInfo::Pinhole(PinholeTreeRoot {
                    parent_tree_root: root_frame,
                    pinhole_projection: pinhole_projection.clone(),
                    parent_root_from_pinhole_root: root_from_current_frame,
                });
                root_frame = transforms.child_frame;

                let previous_root = self.roots.insert(root_frame, new_root_info);
                debug_assert!(previous_root.is_none(), "Root was added already"); // TODO(RR-2667): Build out into cycle detection

                root_from_current_frame = glam::DAffine3::IDENTITY;
            }

            // Collect & compute poses.
            let pose_transforms = &transforms.child_from_instance_poses;
            let root_from_instances =
                compute_root_from_poses(root_from_current_frame, pose_transforms);

            let transform_root_from_current = TransformInfo {
                root: root_frame,
                target_from_source: root_from_current_frame,
                target_from_instances: root_from_instances,
            };

            let previous_transform = self
                .root_from_frame
                .insert(transforms.child_frame, transform_root_from_current);

            // TODO(RR-2667): Build out into cycle detection
            debug_assert!(
                previous_transform.is_none(),
                "Root from frame relationship was added already for {:?}. Now targeting {:?}, previously {:?}",
                cache
                    .frame_id_registry()
                    .lookup_frame_id(transforms.child_frame),
                cache.frame_id_registry().lookup_frame_id(root_frame),
                previous_transform.and_then(|f| cache.frame_id_registry().lookup_frame_id(f.root))
            );

            root_from_target = root_from_current_frame;
        }
    }
}

/// Starting from a `current_frame`, walks towards the parent and accumulates transforms into `transform_stack`.
/// Stops until not more connection is found or an already processed `frame_id` is hit.
fn walk_towards_parent(
    entity_db: &EntityDb,
    query: &LatestAtQuery,
    current_frame: TransformFrameIdHash,
    id_registry: &FrameIdRegistry,
    transforms: &CachedTransformsForTimeline,
    unprocessed_frames: &mut IntSet<TransformFrameIdHash>,
    transform_stack: &mut Vec<ParentChildTransforms>,
) {
    re_tracing::profile_function!();

    debug_assert!(
        transform_stack.is_empty(),
        "Didn't process the last transform stack fully."
    );

    let mut next_frame = Some(current_frame);
    while let Some(current_frame) = next_frame
        && unprocessed_frames.remove(&current_frame)
    {
        // We either already processed this frame, or we reached the end of our path if this source is not in the list of unprocessed frames.
        let mut transforms = transforms_at(current_frame, entity_db, query, transforms);

        // Maybe there's an implicit connection that we have to fill in?
        if transforms.parent_from_child.is_none()
            && let Some(parent) = implicit_transform_parent(current_frame, id_registry)
        {
            transforms.parent_from_child = Some(ParentFromChildTransform {
                parent,
                transform: glam::DAffine3::IDENTITY,
            });
        }

        next_frame = transforms.parent_from_child.as_ref().map(|r| r.parent);

        // No matter the previous outcome, we push the transform information we got about this frame onto the stack
        // since we want something for every source we process.
        transform_stack.push(transforms);
    }
}

/// If `frame` is an implicit transform frame and has a parent, return said parent.
fn implicit_transform_parent(
    frame: TransformFrameIdHash,
    id_registry: &FrameIdRegistry,
) -> Option<TransformFrameIdHash> {
    debug_assert!(
        &id_registry.lookup_frame_id(frame).is_some(),
        "Frame id hash {frame:?} is not known to the cache at all."
    );

    Some(TransformFrameIdHash::from_entity_path(
        &id_registry
            .lookup_frame_id(frame)?
            .as_entity_path()?
            .parent()?,
    ))
}

impl TransformForest {
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
    /// This function computes all T, such that for each source `p_target = T * p_source`
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
        lookup_image_plane_distance: &dyn Fn(TransformFrameIdHash) -> f64,
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
    lookup_image_plane_distance: &dyn Fn(TransformFrameIdHash) -> f64,
    target_from_image_plane_cache: &mut IntMap<TransformFrameIdHash, glam::DAffine3>,
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
    target_from_source_root_cache: &mut IntMap<TransformFrameIdHash, glam::DAffine3>,
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
    target_from_reference: glam::DAffine3,
    reference_from_source: &SmallVec1<[glam::DAffine3; 1]>,
) -> SmallVec1<[glam::DAffine3; 1]> {
    // Easiest to deal with SmallVec1 in-place.
    let mut target_from_source = reference_from_source.clone();
    for transform in &mut target_from_source {
        *transform = target_from_reference * *transform;
    }
    target_from_source
}

fn compute_root_from_poses(
    root_from_entity: glam::DAffine3,
    instance_from_poses: &[glam::DAffine3],
) -> SmallVec1<[glam::DAffine3; 1]> {
    let Ok(mut reference_from_poses) =
        SmallVec1::<[glam::DAffine3; 1]>::try_from_slice(instance_from_poses)
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

fn pinhole3d_from_image_plane(
    resolved_pinhole_projection: &ResolvedPinholeProjection,
    pinhole_image_plane_distance: f64,
) -> glam::DAffine3 {
    let ResolvedPinholeProjection {
        parent: _, // TODO(andreas): Make use of this.
        image_from_camera,
        resolution: _,
        view_coordinates,
    } = resolved_pinhole_projection;

    // Everything under a pinhole camera is a 2D projection, thus doesn't actually have a proper 3D representation.
    // Our visualization interprets this as looking at a 2D image plane from a single point (the pinhole).

    // Center the image plane and move it along z, scaling the further the image plane is.
    let focal_length = image_from_camera.focal_length_in_pixels();
    let focal_length = glam::dvec2(focal_length.x() as f64, focal_length.y() as f64);
    let scale = pinhole_image_plane_distance / focal_length;
    let translation = (glam::DVec2::from(-image_from_camera.principal_point()) * scale)
        .extend(pinhole_image_plane_distance);

    // For anamorphic cameras, use geometric mean for z-scale to balance both dimensions equally
    let z_scale = (scale.x * scale.y).sqrt();

    let image_plane3d_from_2d_content = glam::DAffine3::from_translation(translation)
            // We want to preserve any depth that might be on the pinhole image.
            * glam::DAffine3::from_scale(scale.extend(z_scale));

    // Our interpretation of the pinhole camera implies that the axis semantics, i.e. ViewCoordinates,
    // determine how the image plane is oriented.
    // (see also `CamerasPart` where the frustum lines are set up)
    let obj_from_image_plane3d = glam::DMat3::from_cols_array(
        &view_coordinates
            .from_other(&image_view_coordinates())
            .to_cols_array()
            .map(|x| x as f64),
    );

    glam::DAffine3::from_mat3(obj_from_image_plane3d) * image_plane3d_from_2d_content

    // Above calculation is nice for a certain kind of visualizing a projected image plane,
    // but the image plane distance is arbitrary and there might be other, better visualizations!
}

struct ParentChildTransforms {
    child_frame: TransformFrameIdHash,
    parent_from_child: Option<ParentFromChildTransform>,
    child_from_instance_poses: Vec<glam::DAffine3>,
    pinhole_projection: Option<ResolvedPinholeProjection>,
}

fn transforms_at(
    child_frame: TransformFrameIdHash,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
    transforms_for_timeline: &CachedTransformsForTimeline,
) -> ParentChildTransforms {
    let Some(source_transforms) = transforms_for_timeline.frame_transforms(child_frame) else {
        return ParentChildTransforms {
            child_frame,
            parent_from_child: None,
            child_from_instance_poses: Vec::new(),
            pinhole_projection: None,
        };
    };

    let parent_from_child = source_transforms.latest_at_transform(entity_db, query);
    let child_from_instance_poses = source_transforms.latest_at_instance_poses(entity_db, query);
    let pinhole_projection = source_transforms.latest_at_pinhole(entity_db, query);

    ParentChildTransforms {
        child_frame,
        parent_from_child,
        child_from_instance_poses,
        pinhole_projection,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use itertools::Itertools as _;

    use super::*;

    use crate::TransformFromToError::NoPathBetweenFrames;
    use re_chunk_store::Chunk;
    use re_entity_db::EntityDb;
    use re_log_types::{StoreInfo, TimeCell, TimePoint, TimelineName};
    use re_types::components::TransformFrameId;
    use re_types::{RowId, archetypes};

    fn test_pinhole() -> archetypes::Pinhole {
        archetypes::Pinhole::from_focal_length_and_resolution([1.0, 2.0], [100.0, 200.0])
    }

    /// A test scene that relies exclusively on the entity hierarchy.
    ///
    /// We're using relatively basic transforms here as we assume that resolving transforms have been tested on [`TransformResolutionCache`] already.
    /// Similarly, since [`TransformForest`] does not yet maintain anything over time, we're using static timing instead.
    /// TODO(RR-2510): add another scene (or extension) where we override transforms on select entities
    fn entity_hierarchy_test_scene() -> Result<EntityDb, Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(StoreInfo::testing().store_id);
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(EntityPath::from("top"))
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::from_translation([1.0, 0.0, 0.0]),
                )
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    // Add some instance transforms - we need to make sure they don't propagate.
                    &archetypes::InstancePoses3D::new()
                        .with_translations([[10.0, 0.0, 0.0], [20.0, 0.0, 0.0]]),
                )
                .build()?,
        ))?;
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(EntityPath::from("top/pinhole"))
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::from_translation([0.0, 1.0, 0.0]),
                )
                .with_archetype(RowId::new(), TimePoint::STATIC, &test_pinhole())
                .build()?,
        ))?;

        entity_db.add_chunk(&Arc::new(
            Chunk::builder(EntityPath::from("top/pinhole/child2d"))
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::from_translation([2.0, 0.0, 0.0]),
                )
                .build()?,
        ))?;
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(EntityPath::from("top/child3d"))
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::from_translation([0.0, 0.0, 1.0]),
                )
                .build()?,
        ))?;

        Ok(entity_db)
    }

    fn pretty_print_transform_frame_ids_in<T: std::fmt::Debug>(
        obj: T,
        transform_cache: &TransformResolutionCache,
    ) -> String {
        let mut result = format!("{obj:#?}");
        for (hash, frame) in transform_cache.frame_id_registry().iter_frame_ids() {
            result = result.replace(&format!("{hash:#?}"), &format!("{frame}"));
        }
        result
    }

    #[test]
    fn test_simple_entity_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
        let test_scene = entity_hierarchy_test_scene()?;
        let mut transform_cache = TransformResolutionCache::default();
        transform_cache.add_chunks(test_scene.storage_engine().store().iter_chunks());

        let query = LatestAtQuery::latest(TimelineName::log_tick());
        let transform_forest = TransformForest::new(&test_scene, &transform_cache, &query);

        // Check that we get the expected roots.
        {
            assert_eq!(
                transform_forest.root_info(TransformFrameIdHash::entity_path_hierarchy_root()),
                Some(&TransformTreeRootInfo::TransformFrameRoot)
            );
            // Pinhole roots are a bit more complex. Let's use `insta` to verify.
            insta::assert_snapshot!(
                "simple_entity_hierarchy__root_info_pinhole",
                pretty_print_transform_frame_ids_in(
                    transform_forest.root_info(TransformFrameIdHash::from_entity_path(
                        &EntityPath::from("top/pinhole")
                    )),
                    &transform_cache
                )
            );
            // … but it's hard to reason about the parent root id, so let's verify that just to be sure.
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
                assert_eq!(target_result.target_from_source, glam::DAffine3::IDENTITY);
            } else {
                assert_eq!(
                    target_result.1,
                    Err(TransformFromToError::UnknownTargetFrame(target_frame))
                );
            }

            insta::assert_snapshot!(
                format!("simple_entity_hierarchy__transform_from_to_{}", name),
                pretty_print_transform_frame_ids_in(&result, &transform_cache)
            );
        }

        Ok(())
    }

    /// A test scene that exclusively uses the parent/child farme ids.
    ///
    /// We're using relatively basic transforms here as we assume that resolving transforms have been tested on [`TransformResolutionCache`] already.
    /// Similarly, since [`TransformForest`] does not yet maintain anything over time, we're using static timing instead.
    ///
    /// TODO(RR-2627): Add instances poses into the mix.
    /// TODO(RR-2680): Add pinholes into the mix.
    /// TODO(RR-2799): Variant where everything is just logged on a single entity
    fn simple_frame_hierarchy_test_scene() -> Result<EntityDb, Box<dyn std::error::Error>> {
        let mut entity_db = EntityDb::new(StoreInfo::testing().store_id);
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(EntityPath::from("transforms0"))
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::from_translation([1.0, 0.0, 0.0])
                        .with_child_frame("top")
                        .with_parent_frame("root"),
                )
                .build()?,
        ))?;

        entity_db.add_chunk(&Arc::new(
            Chunk::builder(EntityPath::from("transforms1"))
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::from_translation([2.0, 0.0, 0.0])
                        .with_child_frame("child0")
                        .with_parent_frame("top"),
                )
                .build()?,
        ))?;
        entity_db.add_chunk(&Arc::new(
            Chunk::builder(EntityPath::from("transforms2"))
                .with_archetype_auto_row(
                    TimePoint::STATIC,
                    &archetypes::Transform3D::from_translation([3.0, 0.0, 0.0])
                        .with_child_frame("child1")
                        .with_parent_frame("top"),
                )
                .build()?,
        ))?;

        Ok(entity_db)
    }

    #[test]
    fn test_simple_frame_hierarchy() -> Result<(), Box<dyn std::error::Error>> {
        let test_scene = simple_frame_hierarchy_test_scene()?;
        let mut transform_cache = TransformResolutionCache::default();
        transform_cache.add_chunks(test_scene.storage_engine().store().iter_chunks());

        let query = LatestAtQuery::latest(TimelineName::log_tick());
        let transform_forest = TransformForest::new(&test_scene, &transform_cache, &query);

        // Check that we get the expected roots.
        {
            assert_eq!(
                transform_forest.root_info(TransformFrameIdHash::entity_path_hierarchy_root()),
                Some(&TransformTreeRootInfo::TransformFrameRoot)
            );
            assert_eq!(
                transform_forest.root_info(TransformFrameIdHash::from_str("root")),
                Some(&TransformTreeRootInfo::TransformFrameRoot)
            );
            assert_eq!(transform_forest.roots.len(), 2);
        }

        // Check that there is no connection between the implicit & explicit frames.
        assert_eq!(
            transform_forest
                .transform_from_to(
                    TransformFrameIdHash::from_str("child0"),
                    std::iter::once(TransformFrameIdHash::from_entity_path(
                        &"transforms2".into()
                    )),
                    &|_| 0.0
                )
                .collect_vec(),
            vec![(
                TransformFrameIdHash::from_entity_path(&"transforms2".into()),
                Err(TransformFromToError::NoPathBetweenFrames {
                    target: TransformFrameIdHash::from_str("child0"),
                    src: TransformFrameIdHash::from_entity_path(&"transforms2".into()),
                    target_root: TransformFrameIdHash::from_str("root"),
                    source_root: TransformFrameIdHash::entity_path_hierarchy_root(),
                })
            )]
        );

        // Check that for our two trees everything is connected with everything
        for tree_elements in [
            [
                TransformFrameId::new("top"),
                TransformFrameId::new("root"),
                TransformFrameId::new("child0"),
                TransformFrameId::new("child1"),
            ],
            [
                TransformFrameId::from_entity_path(&"transforms0".into()),
                TransformFrameId::from_entity_path(&"transforms1".into()),
                TransformFrameId::from_entity_path(&"transforms2".into()),
                TransformFrameId::from_entity_path(&EntityPath::root()),
            ],
        ] {
            for pair in tree_elements.iter().permutations(2) {
                let from = pair[0];
                let to = pair[1];
                assert!(
                    matches!(
                        transform_forest
                            .transform_from_to(
                                TransformFrameIdHash::new(from),
                                std::iter::once(TransformFrameIdHash::new(to)),
                                &|_| 1.0
                            )
                            .next(),
                        Some((_, Ok(_)))
                    ),
                    "Connection from {from:?} to {to:?}"
                );
            }
        }

        // Blanket check that we have all the right connections. A bit redundant to above checks, but not as stable due to encompassing snapshotting.
        // We don't test for tree rearrangement here, this has been already tested quite a bit in `test_simple_entity_hierarchy`
        insta::assert_snapshot!(
            "simple_frame_hierarchy_root_connections",
            pretty_print_transform_frame_ids_in(
                &transform_forest.root_from_frame,
                &transform_cache
            )
        );

        Ok(())
    }

    #[test]
    fn test_handling_unknown_frames_gracefully() -> Result<(), Box<dyn std::error::Error>> {
        let query = LatestAtQuery::latest(TimelineName::log_tick());

        // Handle empty store & cache.
        {
            let test_scene = EntityDb::new(StoreInfo::testing().store_id);
            let transform_cache = TransformResolutionCache::default();
            let transform_forest = TransformForest::new(&test_scene, &transform_cache, &query);

            assert_eq!(
                transform_forest
                    .transform_from_to(
                        TransformFrameIdHash::from_str("top"),
                        std::iter::once(TransformFrameIdHash::from_str("child0")),
                        &|_| 1.0
                    )
                    .collect::<Vec<_>>(),
                vec![(
                    TransformFrameIdHash::from_str("child0"),
                    Err(TransformFromToError::UnknownTargetFrame(
                        TransformFrameIdHash::from_str("top")
                    ))
                )]
            );
        }

        // Handle creation from empty cache but full store gracefully.
        {
            let transform_cache = TransformResolutionCache::default();
            let test_scene = simple_frame_hierarchy_test_scene()?;
            let transform_forest = TransformForest::new(&test_scene, &transform_cache, &query);

            // The forest doesn't know about any of the frames despite having seen the populated store.
            assert_eq!(
                transform_forest
                    .transform_from_to(
                        TransformFrameIdHash::from_str("top"),
                        std::iter::once(TransformFrameIdHash::from_str("child0")),
                        &|_| 1.0
                    )
                    .collect::<Vec<_>>(),
                vec![(
                    TransformFrameIdHash::from_str("child0"),
                    Err(TransformFromToError::UnknownTargetFrame(
                        TransformFrameIdHash::from_str("top")
                    ))
                )]
            );
        }

        // Handle creation from partially filled cache gracefully.
        {
            let mut transform_cache = TransformResolutionCache::default();
            let mut test_scene = simple_frame_hierarchy_test_scene()?;
            transform_cache.add_chunks(test_scene.storage_engine().store().iter_chunks());

            // Add a connection the cache doesn't know about.
            test_scene.add_chunk(&Arc::new(
                Chunk::builder(EntityPath::from("transforms"))
                    .with_archetype_auto_row(
                        [(query.timeline(), TimeCell::from_sequence(0))],
                        &archetypes::Transform3D::from_translation([4.0, 0.0, 0.0])
                            .with_child_frame("child2")
                            .with_parent_frame("top"),
                    )
                    .build()?,
            ))?;
            // But the forest seen the scene with it.
            let transform_forest = TransformForest::new(&test_scene, &transform_cache, &query);

            // Forest doesn't know about the newly added `child2` frame.
            assert_eq!(
                transform_forest
                    .transform_from_to(
                        TransformFrameIdHash::from_str("top"),
                        std::iter::once(TransformFrameIdHash::from_str("child2")),
                        &|_| 1.0
                    )
                    .collect::<Vec<_>>(),
                vec![(
                    TransformFrameIdHash::from_str("child2"),
                    Err(TransformFromToError::UnknownSourceFrame(
                        TransformFrameIdHash::from_str("child2")
                    ))
                )]
            );
        }

        // Extra nasty case: given a cold cache, the cache knows about everything except for a row _on the same time_ which talks about a new frame.
        // (this also makes sure that we get the right transform back for the known frames even when a latest-at query would yield something the cache doesn't know about)
        {
            let mut transform_cache = TransformResolutionCache::default();
            let mut test_scene = simple_frame_hierarchy_test_scene()?;

            test_scene.add_chunk(&Arc::new(
                // Add a connection the cache doesn't know about.
                Chunk::builder(EntityPath::from("transforms"))
                    .with_archetype_auto_row(
                        [(query.timeline(), TimeCell::from_sequence(0))],
                        &archetypes::Transform3D::from_translation([4.0, 0.0, 0.0])
                            .with_child_frame("child2")
                            .with_parent_frame("top"),
                    )
                    .build()?,
            ))?;
            transform_cache.add_chunks(test_scene.storage_engine().store().iter_chunks());

            test_scene.add_chunk(&Arc::new(
                // Add a connection the cache doesn't know about.
                Chunk::builder(EntityPath::from("transforms"))
                    .with_archetype_auto_row(
                        [(query.timeline(), TimeCell::from_sequence(0))], // Same time before, different parent frame!
                        &archetypes::Transform3D::from_translation([5.0, 0.0, 0.0])
                            .with_child_frame("child2")
                            .with_parent_frame("new_top"),
                    )
                    .build()?,
            ))?;
            let transform_forest = TransformForest::new(&test_scene, &transform_cache, &query);

            // Forest should know about the old relationship, but not the new.
            assert_eq!(
                transform_forest
                    .transform_from_to(
                        TransformFrameIdHash::from_str("child2"),
                        std::iter::once(TransformFrameIdHash::from_str("top")),
                        &|_| 1.0
                    )
                    .collect::<Vec<_>>(),
                vec![(
                    TransformFrameIdHash::from_str("top"),
                    Err(NoPathBetweenFrames {
                        target: TransformFrameIdHash::from_str("child2"),
                        src: TransformFrameIdHash::from_str("top"),
                        target_root: TransformFrameIdHash::from_str("new_top"),
                        source_root: TransformFrameIdHash::from_str("root"),
                    }) // TODO(RR-2897): this is broken right now. This is what the result should be:
                       // Ok(TransformInfo {
                       //     root: TransformFrameIdHash::from_str("top"),
                       //     target_from_source: glam::DAffine3::from_translation(glam::dvec3(
                       //         4.0, 0.0, 0.0
                       //     )),
                       //     target_from_instances: Default::default(),
                       //     target_from_archetype: Default::default(),
                       // })
                )]
            );
            assert_eq!(
                transform_forest
                    .transform_from_to(
                        TransformFrameIdHash::from_str("child2"),
                        std::iter::once(TransformFrameIdHash::from_str("new_top")),
                        &|_| 1.0
                    )
                    .collect::<Vec<_>>(),
                vec![(
                    TransformFrameIdHash::from_str("new_top"),
                    Ok(TransformInfo {
                        root: TransformFrameIdHash::from_str("new_top"),
                        target_from_source: glam::DAffine3::from_translation(glam::dvec3(
                            -5.0, 0.0, 0.0
                        )),
                        target_from_instances: SmallVec1::new(glam::DAffine3::from_translation(
                            glam::dvec3(-5.0, 0.0, 0.0)
                        )),
                    })
                )] // TODO(RR-2897): this is broken right now. This is what the result should be:
                   // vec![(
                   //     TransformFrameIdHash::from_str("child2"),
                   //     Err(TransformFromToError::UnknownTargetFrame(
                   //         TransformFrameIdHash::from_str("new_top")
                   //     ))
                   // )]
            );
        }

        Ok(())
    }
}
