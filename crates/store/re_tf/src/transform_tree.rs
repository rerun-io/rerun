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

// TODO(andreas): this struct is comically large for what we're doing here. Need to refactor this to make it smaller & more efficient.
#[derive(Clone, Debug)]
pub struct TransformInfo {
    /// The transform from the entity to the reference space.
    ///
    /// ⚠️ Does not include per instance poses! ⚠️
    /// Include 3D-from-2D / 2D-from-3D pinhole transform if present.
    reference_from_entity: glam::Affine3A,

    /// List of transforms per instance including poses.
    ///
    /// If no poses are present, this is always the same as `reference_from_entity`.
    /// (also implying that in this case there is only a single element).
    /// If there are poses there may be more than one element.
    ///
    /// Does not take into account archetype specific transforms.
    reference_from_instances_overall: SmallVec1<[glam::Affine3A; 1]>,

    /// Like [`Self::reference_from_instances_overall`] but _on top_ also has archetype specific transforms applied
    /// if there are any present.
    reference_from_archetype: IntMap<ArchetypeName, SmallVec1<[glam::Affine3A; 1]>>,

    /// If this entity is under (!) a pinhole camera, this contains additional information.
    ///
    /// TODO(#2663, #1025): Going forward we should have separate transform hierarchies for 2D (i.e. projected) and 3D,
    /// which would remove the need for this.
    pub twod_in_threed_info: Option<TwoDInThreeDTransformInfo>,
}

#[derive(Clone, Debug)]
pub struct TwoDInThreeDTransformInfo {
    /// Pinhole camera ancestor (may be this entity itself).
    ///
    /// None indicates that this entity is under the eye camera with no Pinhole camera in-between.
    /// Some indicates that the entity is under a pinhole camera at the given entity path that is not at the root of the view.
    pub parent_pinhole: EntityPath,

    /// The last 3D from 3D transform at the pinhole camera, before the pinhole transformation itself.
    pub reference_from_pinhole_entity: glam::Affine3A,
}

impl Default for TransformInfo {
    fn default() -> Self {
        Self {
            reference_from_entity: glam::Affine3A::IDENTITY,
            reference_from_instances_overall: SmallVec1::new(glam::Affine3A::IDENTITY),
            reference_from_archetype: Default::default(),
            twod_in_threed_info: None,
        }
    }
}

impl TransformInfo {
    /// Warns that multiple transforms within the entity are not supported.
    #[inline]
    fn warn_on_per_instance_transform(&self, entity_name: &EntityPath, archetype: ArchetypeName) {
        if self.reference_from_instances_overall.len() > 1 {
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

        if let Some(transform) = self.reference_from_archetype.get(&archetype) {
            *transform.first()
        } else {
            *self.reference_from_instances_overall.first()
        }
    }

    /// Returns reference from instance transforms.
    #[inline]
    pub fn reference_from_instances(
        &self,
        archetype: ArchetypeName,
    ) -> &SmallVec1<[glam::Affine3A; 1]> {
        if let Some(transform) = self.reference_from_archetype.get(&archetype) {
            transform
        } else {
            &self.reference_from_instances_overall
        }
    }

    /// Multiplies all transforms from the left by `target_from_reference`
    ///
    /// Or in other words:
    /// `reference_from_source = self`
    /// `target_from_source = target_from_reference * reference_from_source`
    ///
    /// ⚠️ does not affect 2D-in-3D information, leaving it unaffected entirely.
    pub fn left_multiply(&self, target_from_reference: glam::Affine3A) -> Self {
        let Self {
            reference_from_entity: reference_from_source,
            reference_from_instances_overall: reference_from_source_instances_overall,
            reference_from_archetype: reference_from_source_archetypes,
            twod_in_threed_info: twod_in_threed_info_source,
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
            reference_from_entity: target_from_source,
            reference_from_instances_overall: target_from_source_instances_overall,
            reference_from_archetype: target_from_source_archetypes,
            twod_in_threed_info: twod_in_threed_info_source.clone(),
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum TransformFromToError {
    #[error("No transform relationships about the target frame {0:?} are known")]
    UnknownTargetFrame(EntityPathHash),

    #[error("No transform relationships about the source frame {0:?} are known")]
    UnknownSourceFrame(EntityPathHash),
    // TODO(RR-2514): Can't happen yet since for now everything is connected to the root.
    // #[error("There's no path between {target:?} and {source:?}")]
    // NoPathBetweenFrames {
    //     target: EntityPathHash,
    //     source: EntityPathHash,
    // },
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
// TODO(andreas): This has to become a `TransformForest` as we move on to arbitrary graphs.
#[derive(Clone)]
pub struct TransformTree {
    /// All entities reachable from the root.
    transform_per_entity: IntMap<EntityPathHash, TransformInfo>,
}

impl TransformTree {
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
            transform_per_entity: Default::default(),
        };
        tree.gather_descendants_transforms(
            entity_tree,
            time_query,
            // Ignore potential pinhole camera at the root of the view, since it is regarded as being "above" this root.
            TransformInfo::default(),
            transforms,
        );

        tree
    }
}

impl TransformTree {
    #[allow(clippy::too_many_arguments)]
    fn gather_descendants_transforms(
        &mut self,
        subtree: &EntityTree,
        query: &LatestAtQuery,
        transform: TransformInfo,
        transforms_for_timeline: &CachedTransformsForTimeline,
    ) {
        let twod_in_threed_info = transform.twod_in_threed_info.clone();
        let reference_from_parent = transform.reference_from_entity;
        match self.transform_per_entity.entry(subtree.path.hash()) {
            std::collections::hash_map::Entry::Occupied(_) => {
                return;
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(transform);
            }
        }

        for child_tree in subtree.children.values() {
            let child_path = &child_tree.path;

            let mut encountered_pinhole = twod_in_threed_info
                .as_ref()
                .map(|info| info.parent_pinhole.clone());

            let transforms_at_entity = transforms_at(
                child_path,
                query,
                &mut encountered_pinhole,
                transforms_for_timeline,
            );
            let new_transform = transform_info_for_downward_propagation(
                child_path,
                reference_from_parent,
                twod_in_threed_info.clone(),
                &transforms_at_entity,
            );

            self.gather_descendants_transforms(
                child_tree,
                query,
                new_transform,
                transforms_for_timeline,
            );
        }
    }

    /// Computes the transform from one entity to another if there is a path between them.
    ///
    /// `target`: The frame into which to transform.
    /// `sources`: The frames from which to transform.
    ///
    /// Returns an iterator of results, one for each source.
    /// If the target frame is not known at all, returns [`TransformFromToError::UnknownTargetFrame`] for every source.
    pub fn transform_from_to(
        &self,
        target: EntityPathHash,
        sources: impl Iterator<Item = EntityPathHash>,
    ) -> impl Iterator<Item = (EntityPathHash, Result<TransformInfo, TransformFromToError>)> {
        let Some(reference_from_target) = self.transform_per_entity.get(&target) else {
            return itertools::Either::Left(sources.map(move |source| {
                (
                    source,
                    Err(TransformFromToError::UnknownTargetFrame(target)),
                )
            }));
        };

        // Invert `reference_from_target` to get `target_from_reference`.
        let target_from_reference = {
            let TransformInfo {
                reference_from_entity,
                // Don't care about instance transforms on the target frame, as they don't tree-propagate.
                reference_from_instances_overall: _,
                // Don't care about archetype specific transforms on the target frame, as they don't tree-propagate.
                reference_from_archetype: _,
                // It's called "2D in 3D", therefore the inverse would be "3D in 2D".
                // Which could be valuable information, but not something we're capturing right now!
                twod_in_threed_info: _,
            } = &reference_from_target;

            reference_from_entity.inverse()
        };

        itertools::Either::Right(sources.map(move |source| {
            let Some(reference_from_source) = self.transform_per_entity.get(&source) else {
                return (
                    source,
                    Err(TransformFromToError::UnknownSourceFrame(source)),
                );
            };

            // target_from_source = target_from_reference * reference_from_source
            let target_from_source = reference_from_source.left_multiply(target_from_reference);
            (source, Ok(target_from_source))
        }))
    }
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
}

fn compute_reference_from_instances(
    reference_from_entity: glam::Affine3A,
    instance_from_poses: &[glam::Affine3A],
) -> SmallVec1<[glam::Affine3A; 1]> {
    let Ok(mut reference_from_poses) =
        SmallVec1::<[glam::Affine3A; 1]>::try_from_slice(instance_from_poses)
    else {
        return SmallVec1::new(reference_from_entity);
    };

    // Until now `reference_from_poses` is actually `reference_from_entity`.
    for reference_from_pose in &mut reference_from_poses {
        let entity_from_pose = *reference_from_pose;
        *reference_from_pose = reference_from_entity * entity_from_pose;
    }
    reference_from_poses
}

fn compute_references_from_instances_overall(
    reference_from_entity: glam::Affine3A,
    pose_transforms: Option<&PoseTransformArchetypeMap>,
) -> SmallVec1<[glam::Affine3A; 1]> {
    compute_reference_from_instances(
        reference_from_entity,
        pose_transforms.map_or(&[], |poses| &poses.instance_from_overall_poses),
    )
}

fn compute_reference_from_archetype(
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
                        compute_reference_from_instances(reference_from_entity, poses),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Compute transform info for when we walk down the tree from the reference.
fn transform_info_for_downward_propagation(
    current_path: &EntityPath,
    reference_from_parent: glam::Affine3A,
    mut twod_in_threed_info: Option<TwoDInThreeDTransformInfo>,
    transforms_at_entity: &TransformsAtEntity<'_>,
) -> TransformInfo {
    let mut reference_from_entity = reference_from_parent;

    // Apply tree transform.
    reference_from_entity *= transforms_at_entity.parent_from_entity_tree_transform;

    // Apply 2D->3D transform if present.
    if let Some(entity_from_2d_pinhole_content) =
        transforms_at_entity.instance_from_pinhole_image_plane
    {
        // Should have bailed out already earlier.
        debug_assert!(
            twod_in_threed_info.is_none(),
            "2D->3D transform already set, this should be unreachable."
        );

        twod_in_threed_info = Some(TwoDInThreeDTransformInfo {
            parent_pinhole: current_path.clone(),
            reference_from_pinhole_entity: reference_from_entity,
        });
        reference_from_entity *= entity_from_2d_pinhole_content;
    }

    // Collect & compute poses.
    let reference_from_instances_overall = compute_references_from_instances_overall(
        reference_from_entity,
        transforms_at_entity.entity_from_instance_poses,
    );
    let reference_from_archetype = compute_reference_from_archetype(
        reference_from_entity,
        transforms_at_entity.entity_from_instance_poses,
    );

    TransformInfo {
        reference_from_entity,
        reference_from_instances_overall,
        reference_from_archetype,
        twod_in_threed_info,
    }
}

fn transform_from_pinhole_with_image_plane(
    resolved_pinhole_projection: &ResolvedPinholeProjection,
    pinhole_image_plane_distance: f32,
) -> glam::Affine3A {
    let ResolvedPinholeProjection {
        image_from_camera,
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
    instance_from_pinhole_image_plane: Option<glam::Affine3A>,
}

fn transforms_at<'a>(
    entity_path: &EntityPath,
    query: &LatestAtQuery,
    encountered_pinhole: &mut Option<EntityPath>,
    transforms_for_timeline: &'a CachedTransformsForTimeline,
) -> TransformsAtEntity<'a> {
    // This is called very frequently, don't put a profile scope here.

    let Some(entity_transforms) = transforms_for_timeline.entity_transforms(entity_path) else {
        return TransformsAtEntity::default();
    };

    let parent_from_entity_tree_transform = entity_transforms.latest_at_tree_transform(query);
    let entity_from_instance_poses = entity_transforms.latest_at_instance_poses_all(query);
    let instance_from_pinhole_image_plane =
        entity_transforms
            .latest_at_pinhole(query)
            .map(|resolved_pinhole_projection| {
                transform_from_pinhole_with_image_plane(
                    resolved_pinhole_projection,
                    1.0, // TODO: I think we can just scale this later...
                )
            });

    let transforms_at_entity = TransformsAtEntity {
        parent_from_entity_tree_transform,
        entity_from_instance_poses,
        instance_from_pinhole_image_plane,
    };

    // Handle pinhole encounters.
    if transforms_at_entity
        .instance_from_pinhole_image_plane
        .is_some()
    {
        *encountered_pinhole = Some(entity_path.clone());
    }

    transforms_at_entity
}

// TODO: unit tests?
