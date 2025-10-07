use glam::Affine3A;
use nohash_hasher::IntMap;
use vec1::smallvec_v1::SmallVec1;

use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityPath, EntityTree};
use re_log_types::EntityPathHash;
use re_types::ArchetypeName;

use crate::{
    CachedTransformsForTimeline, PoseTransformArchetypeMap, ResolvedPinholeProjection,
    TransformResolutionCache, image_view_coordinates,
};

/// Transform from an entity towards a given root.
#[derive(Clone, Debug)]
pub struct TransformInfo {
    /// Root this transform belongs to.
    ///
    /// ⚠️ This is the root of the tree this transform belongs to,
    /// not necessarily what the transform transforms into.
    root: EntityPathHash,

    /// The transform from the entity to the root's space.
    ///
    /// ⚠️ Does not include per instance poses! ⚠️
    /// Include 3D-from-2D / 2D-from-3D pinhole transform if present.
    target_from_entity: glam::Affine3A,

    /// List of transforms per instance including poses.
    ///
    /// If no poses are present, this is always the same as `root_from_entity`.
    /// (also implying that in this case there is only a single element).
    /// If there are poses there may be more than one element.
    ///
    /// Does not take into account archetype specific transforms.
    target_from_instances_overall: SmallVec1<[glam::Affine3A; 1]>,

    /// Like [`Self::root_from_instances_overall`] but _on top_ also has archetype specific transforms applied
    /// if there are any present.
    target_from_archetype: IntMap<ArchetypeName, SmallVec1<[glam::Affine3A; 1]>>,
}

impl TransformInfo {
    fn new_root(root: EntityPathHash) -> Self {
        Self {
            root,
            target_from_entity: glam::Affine3A::IDENTITY,
            target_from_instances_overall: SmallVec1::new(glam::Affine3A::IDENTITY),
            target_from_archetype: Default::default(),
        }
    }

    /// Returns the root of the tree this transform belongs to.
    ///
    /// This is **not** necessarily the transform's target space.
    #[inline]
    pub fn tree_root(&self) -> EntityPathHash {
        self.root
    }

    /// Warns that multiple transforms within the entity are not supported.
    #[inline]
    fn warn_on_per_instance_transform(&self, entity_name: &EntityPath, archetype: ArchetypeName) {
        if self.target_from_instances_overall.len() > 1 {
            re_log::warn_once!(
                "There are multiple poses for entity {entity_name:?}. {archetype:?} supports only one transform per entity. Using the first one."
            );
        }
    }

    /// Returns the first instance transform and warns if there are multiple.
    #[inline]
    pub fn single_entity_transform_required(
        &self,
        entity_name: &EntityPath,
        archetype: ArchetypeName,
    ) -> glam::Affine3A {
        self.warn_on_per_instance_transform(entity_name, archetype);

        if let Some(transform) = self.target_from_archetype.get(&archetype) {
            *transform.first()
        } else {
            *self.target_from_instances_overall.first()
        }
    }

    /// Returns reference from instance transforms.
    #[inline]
    pub fn reference_from_instances(
        &self,
        archetype: ArchetypeName,
    ) -> &SmallVec1<[glam::Affine3A; 1]> {
        if let Some(transform) = self.target_from_archetype.get(&archetype) {
            transform
        } else {
            &self.target_from_instances_overall
        }
    }

    /// Multiplies all transforms from the left by `target_from_reference`
    ///
    /// Or in other words:
    /// `reference_from_source = self`
    /// `target_from_source = target_from_reference * reference_from_source`
    pub fn left_multiply(&self, target_from_reference: glam::Affine3A) -> Self {
        let Self {
            root,
            target_from_entity: reference_from_source,
            target_from_instances_overall: reference_from_source_instances_overall,
            target_from_archetype: reference_from_source_archetypes,
        } = self;

        let target_from_source = target_from_reference * reference_from_source;
        let target_from_source_instances_overall = left_multiply_smallvec1_of_transforms(
            target_from_reference,
            reference_from_source_instances_overall,
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
            target_from_entity: target_from_source,
            target_from_instances_overall: target_from_source_instances_overall,
            target_from_archetype: target_from_source_archetypes,
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum TransformFromToError {
    #[error("No transform relationships about the target frame {0:?} are known")]
    UnknownTargetFrame(EntityPathHash),

    #[error("No transform relationships about the source frame {0:?} are known")]
    UnknownSourceFrame(EntityPathHash),

    #[error(
        "There's no path between {target:?} and {src:?}. The target's root is {target_root:?}, the source's root is {source_root:?}"
    )]
    NoPathBetweenFrames {
        target: EntityPathHash,
        src: EntityPathHash, // Can't name this `source` for some strange procmacro reasons
        target_root: EntityPathHash,
        source_root: EntityPathHash,
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
    id: EntityPathHash,
    root: EntityPathHash,
    target_from_root: glam::Affine3A,
}

/// Private utility struct for working with a source frame.
struct SourceInfo<'a> {
    id: EntityPathHash,
    root: EntityPathHash,
    root_from_source: &'a TransformInfo,
}

/// Properties of a pinhole transform tree root.
///
/// Each pinhole forms its own subtree which may be embedded into a 3D space.
/// Everything at and below the pinhole tree root is considered to be 2D,
/// everything above is considered to be 3D.
#[derive(Clone, Debug)]
pub struct PinholeTreeRoot {
    /// The tree root of the parent of this pinhole.
    pub parent_tree_root: EntityPathHash,

    /// Pinhole projection that defines how 2D objects are transformed in this space.
    pub pinhole_projection: ResolvedPinholeProjection,

    /// Transforms the 2D subtree into its parent 3D space.
    pub parent_root_from_pinhole_root: glam::Affine3A,
}

/// Properties of a transform root.
///
/// [`TransformForest`] tries to identify all roots.
#[derive(Clone, Debug)]
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

/// Provides transforms from any transform frame to a root transform frame for a given time & timeline.
///
/// This information can then be used to relatively quickly resolve transforms to a given reference transform frame
/// should such a connection exist.
///
/// The resulting transforms are dependent on:
/// * tree, pose, pinhole and view-coordinates transforms components as logged to the data store. See [`TransformResolutionCache`] for more details.
///    * TODO(#6743): blueprint overrides aren't respected yet
/// * the query time
///    * TODO(#723): ranges aren't taken into account yet
// TODO: update docs a bit
#[derive(Default, Clone)]
pub struct TransformForest {
    /// All known tree roots.
    roots: IntMap<EntityPathHash, TransformTreeRootInfo>,

    /// All entities reachable from one of the tree roots.
    root_from_entity: IntMap<EntityPathHash, TransformInfo>,
}

impl TransformForest {
    /// Determines transforms for all entities relative to a space path which serves as the "reference".
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

        let mut tree = Self {
            roots: std::iter::once((EntityPath::root().hash(), TransformTreeRootInfo::EntityRoot))
                .collect(),
            root_from_entity: Default::default(),
        };
        tree.gather_descendants_transforms(
            entity_tree,
            time_query,
            // Ignore potential pinhole camera at the root of the view, since it is regarded as being "above" this root.
            // TODO(andreas): Should we warn about that?
            TransformInfo::new_root(EntityPath::root().hash()),
            transforms,
        );

        tree
    }
}

impl TransformForest {
    #[allow(clippy::too_many_arguments)]
    fn gather_descendants_transforms(
        &mut self,
        subtree: &EntityTree,
        query: &LatestAtQuery,
        transform_root_from_parent: TransformInfo,
        transforms_for_timeline: &CachedTransformsForTimeline,
    ) {
        let root = transform_root_from_parent.root;
        let root_from_parent = transform_root_from_parent.target_from_entity;

        let previous_transform = self
            .root_from_entity
            .insert(subtree.path.hash(), transform_root_from_parent);
        debug_assert!(previous_transform.is_none(), "Root was added already"); // TODO(andreas): Build out into cycle detection (cycles can't _yet_ happen)

        for child_tree in subtree.children.values() {
            let child_path = &child_tree.path;

            let transforms_at_entity = transforms_at(child_path, query, transforms_for_timeline);

            let root_from_child =
                root_from_parent * transforms_at_entity.parent_from_entity_tree_transform;

            // Did we encounter a pinhole and need to create a new subspace?
            // TODO: report nested pinholes
            let (root, root_from_entity) =
                if let Some(pinhole_projection) = transforms_at_entity.pinhole_projection {
                    let new_root = child_path.hash();
                    let new_root_info = TransformTreeRootInfo::Pinhole(PinholeTreeRoot {
                        parent_tree_root: root,
                        pinhole_projection: pinhole_projection.clone(),
                        parent_root_from_pinhole_root: root_from_child,
                    });

                    let previous_root = self.roots.insert(new_root, new_root_info);
                    debug_assert!(previous_root.is_none(), "Root was added already"); // TODO(andreas): Build out into cycle detection (cycles can't _yet_ happen)

                    (new_root, Affine3A::IDENTITY)
                } else {
                    (root, root_from_child)
                };

            // Collect & compute poses.
            let root_from_instances_overall = compute_root_from_instances_overall(
                root_from_entity,
                transforms_at_entity.entity_from_instance_poses,
            );
            let root_from_archetype = compute_root_from_archetype(
                root_from_entity,
                transforms_at_entity.entity_from_instance_poses,
            );

            let transform_root_from_child = TransformInfo {
                root,
                target_from_entity: root_from_entity,
                target_from_instances_overall: root_from_instances_overall,
                target_from_archetype: root_from_archetype,
            };

            self.gather_descendants_transforms(
                child_tree,
                query,
                transform_root_from_child,
                transforms_for_timeline,
            );
        }
    }

    /// Returns the properties of the pinhole tree root at the given entity path if the entity's root is a pinhole tree root.
    #[inline]
    pub fn pinhole_tree_root_info(&self, root: EntityPathHash) -> Option<&PinholeTreeRoot> {
        if let TransformTreeRootInfo::Pinhole(pinhole_tree_root) = self.roots.get(&root)? {
            Some(pinhole_tree_root)
        } else {
            None
        }
    }

    /// Computes the transform from one entity to another if there is a path between them.
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
        target: EntityPathHash,
        sources: impl Iterator<Item = EntityPathHash>,
        lookup_image_plane_distance: &dyn Fn(EntityPathHash) -> f32,
    ) -> impl Iterator<Item = (EntityPathHash, Result<TransformInfo, TransformFromToError>)> {
        // We're looking for a common root between source and target.
        // We start by looking up the target's tree root.

        let Some(root_from_target) = self.root_from_entity.get(&target) else {
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
                target_from_entity: root_from_entity,
                // Don't care about instance transforms on the target frame, as they don't tree-propagate.
                target_from_instances_overall: _,
                // Don't care about archetype specific transforms on the target frame, as they don't tree-propagate.
                target_from_archetype: _,
            } = &root_from_target;

            TargetInfo {
                id: target,
                root: *target_root,
                target_from_root: root_from_entity.inverse(),
            }
        };

        itertools::Either::Right(sources.map(move |source| {
            let Some(root_from_source) = self.root_from_entity.get(&source) else {
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
                // TODO: fast track for root being target.
                // target_from_source = target_from_reference * root_from_source
                let target_from_source = root_from_source.left_multiply(target.target_from_root);
                Ok(target_from_source)
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
    pinhole_tree_root: &PinholeTreeRoot,
    lookup_image_plane_distance: &dyn Fn(EntityPathHash) -> f32,
) -> Result<TransformInfo, TransformFromToError> {
    let PinholeTreeRoot {
        parent_tree_root,
        pinhole_projection,
        parent_root_from_pinhole_root: root_from_pinhole3d,
    } = pinhole_tree_root;

    if *parent_tree_root != target.root {
        return Err(TransformFromToError::no_path_between_target_and_source(
            target, source,
        ));
    }

    // Rename for clarification:
    let pinhole2d_from_source = source.root_from_source;

    // There's a connection via a pinhole making this 2D in 3D.
    // We can transform into the target space!
    let pinhole_image_plane_distance = lookup_image_plane_distance(source.root);

    // TODO: Locally cache pinhole transform?
    let pinhole3d_from_pinhole2d =
        pinhole2d_image_plane_from_pinhole3d(pinhole_projection, pinhole_image_plane_distance);
    let target_from_pinhole2d =
        target.target_from_root * root_from_pinhole3d * pinhole3d_from_pinhole2d;

    // target_from_source = target_from_pinhole2d * pinhole2d_from_source
    Ok(pinhole2d_from_source.left_multiply(target_from_pinhole2d))
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

fn compute_root_from_instances(
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

fn compute_root_from_instances_overall(
    reference_from_entity: glam::Affine3A,
    pose_transforms: Option<&PoseTransformArchetypeMap>,
) -> SmallVec1<[glam::Affine3A; 1]> {
    compute_root_from_instances(
        reference_from_entity,
        pose_transforms.map_or(&[], |poses| &poses.instance_from_overall_poses),
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
                        compute_root_from_instances(reference_from_entity, poses),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn pinhole2d_image_plane_from_pinhole3d(
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

    // TODO(#1025):
    // As such we don't ever want to invert this matrix!
    // However, currently our 2D views require us to do exactly that since we're forced to
    // build a relationship between the 2D plane and the 3D world, when actually the 2D plane
    // should have infinite depth!
    // The inverse of this matrix *is* working for this, but quickly runs into precision issues.
    // See also `ui_2d.rs#setup_target_config`
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

// TODO: unit tests?
// TODO: need 2d in 3d tests. don't think we have any
// TODO: need 3d in 2d tests. don't think we have any
