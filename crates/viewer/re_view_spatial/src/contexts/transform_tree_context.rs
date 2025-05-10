use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityPath, EntityTree};
use re_log_types::EntityPathHash;
use re_types::{
    Archetype as _, Component as _, ComponentDescriptorSet,
    archetypes::{self, InstancePoses3D, Transform3D},
    components::ImagePlaneDistance,
};
use re_view::DataResultQuery as _;
use re_viewer_context::{DataResultTree, IdentifiedViewSystem, ViewContext, ViewContextSystem};
use vec1::smallvec_v1::SmallVec1;

use crate::{
    transform_cache::{
        CachedTransformsForTimeline, ResolvedPinholeProjection, TransformCacheStoreSubscriber,
    },
    visualizers::image_view_coordinates,
};

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
    pub reference_from_instances: SmallVec1<[glam::Affine3A; 1]>,

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
            reference_from_instances: SmallVec1::new(glam::Affine3A::IDENTITY),
            twod_in_threed_info: None,
        }
    }
}

impl TransformInfo {
    /// Warns that multiple transforms within the entity are not supported.
    #[inline]
    pub fn warn_on_per_instance_transform(
        &self,
        entity_name: &EntityPath,
        visualizer_name: &'static str,
    ) {
        if self.reference_from_instances.len() > 1 {
            re_log::warn_once!(
                "There are multiple poses for entity {entity_name:?}. Visualizer {visualizer_name:?} supports only one transform per entity. Using the first one."
            );
        }
    }

    /// Returns the first instance transform and warns if there are multiple (via [`Self::warn_on_per_instance_transform`]).
    #[inline]
    pub fn single_entity_transform_required(
        &self,
        entity_name: &EntityPath,
        visualizer_name: &'static str,
    ) -> glam::Affine3A {
        self.warn_on_per_instance_transform(entity_name, visualizer_name);
        *self.reference_from_instances.first()
    }

    /// Returns the first instance transform and does not warn if there are multiple.
    #[inline]
    pub fn single_entity_transform_silent(&self) -> glam::Affine3A {
        *self.reference_from_instances.first()
    }

    /// Returns reference from instance transforms, repeating the last value indefinitely.
    #[inline]
    pub fn clamped_reference_from_instances(&self) -> impl Iterator<Item = glam::Affine3A> + '_ {
        self.reference_from_instances
            .iter()
            .chain(std::iter::repeat(self.reference_from_instances.last()))
            .copied()
    }
}

/// Provides transforms from an entity to a chosen reference space for all elements in the scene
/// for the currently selected time & timeline.
///
/// The resulting transforms are dependent on:
/// * tree, pose, pinhole and view-coordinates transforms components as logged to the data store
///    * TODO(#6743): blueprint overrides aren't respected yet
/// * the view' spatial origin
/// * the query time
///    * TODO(#723): ranges aren't taken into account yet
/// * TODO(andreas): the queried entities. Right now we determine transforms for ALL entities in the scene.
///                  since 3D views tend to display almost everything that's mostly fine, but it's very wasteful when they don't.
///
/// The renderer then uses this reference space as its world space,
/// making world and reference space equivalent for a given view.
///
/// TODO(#7025): Right now we also do full tree traversal in here to resolve transforms to the root.
/// However, for views that share the same query, we can easily make all entities relative to the respective origin in a linear pass over all matrices.
/// (Note that right now the query IS always the same across all views for a given frame since it's just latest-at controlled by the timeline,
/// but once we support range queries it may be not or only partially the case)
#[derive(Clone)]
pub struct TransformTreeContext {
    /// All transforms provided are relative to this reference path.
    space_origin: EntityPath,

    /// All reachable entities.
    transform_per_entity: IntMap<EntityPathHash, TransformInfo>,
}

impl IdentifiedViewSystem for TransformTreeContext {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TransformContext".into()
    }
}

impl Default for TransformTreeContext {
    fn default() -> Self {
        Self {
            space_origin: EntityPath::root(),
            transform_per_entity: Default::default(),
        }
    }
}

impl ViewContextSystem for TransformTreeContext {
    fn compatible_component_sets(&self) -> Vec<ComponentDescriptorSet> {
        vec![
            Transform3D::all_components().iter().cloned().collect(),
            InstancePoses3D::all_components().iter().cloned().collect(),
            std::iter::once(archetypes::Pinhole::descriptor_image_from_camera()).collect(),
        ]
    }

    /// Determines transforms for all entities relative to a space path which serves as the "reference".
    /// I.e. the resulting transforms are "reference from scene"
    ///
    /// This means that the entities in `reference_space` get the identity transform and all other
    /// entities are transformed relative to it.
    fn execute(
        &mut self,
        ctx: &re_viewer_context::ViewContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
    ) {
        re_tracing::profile_function!();
        debug_assert_transform_field_order(ctx.viewer_ctx.reflection());

        // Make sure transform cache is up to date.
        // TODO(andreas): This is a rather annoying sync point between different views.
        // We could alleviate this by introducing a per view class (not instance) method that is called
        // before system execution.
        TransformCacheStoreSubscriber::access_mut(&ctx.recording().store_id(), |cache| {
            cache.apply_all_updates(ctx.recording());
        });

        let entity_tree = ctx.recording().tree();
        let query_result = ctx.viewer_ctx.lookup_query_result(query.view_id);
        let data_result_tree = &query_result.tree;

        self.space_origin = query.space_origin.clone();

        // Find the entity path tree for the root.
        let Some(current_tree) = &entity_tree.subtree(query.space_origin) else {
            // It seems the space path is not part of the object tree!
            // This happens frequently when the viewer remembers views from a previous run that weren't shown yet.
            // Naturally, in this case we don't have any transforms yet.
            return;
        };

        let time_query = ctx.current_query();

        TransformCacheStoreSubscriber::access(&ctx.recording().store_id(), |cache| {
            let transforms = cache.transforms_for_timeline(query.timeline);

            // Child transforms of this space
            {
                re_tracing::profile_scope!("gather_descendants_transforms");

                self.gather_descendants_transforms(
                    ctx,
                    data_result_tree,
                    current_tree,
                    &time_query,
                    // Ignore potential pinhole camera at the root of the view, since it is regarded as being "above" this root.
                    TransformInfo::default(),
                    transforms,
                );
            }

            // Walk up from the reference to the highest reachable parent.
            self.gather_parent_transforms(
                ctx,
                data_result_tree,
                current_tree,
                &time_query,
                transforms,
            );
        }); // Note that this can return None if no event has happened for this timeline yet.
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TransformTreeContext {
    /// Gather transforms for everything _above_ the root.
    fn gather_parent_transforms<'a>(
        &mut self,
        ctx: &'a ViewContext<'a>,
        data_result_tree: &DataResultTree,
        mut current_tree: &'a EntityTree,
        time_query: &LatestAtQuery,
        transforms: &CachedTransformsForTimeline,
    ) {
        re_tracing::profile_function!();

        let entity_tree = ctx.recording().tree();

        let mut reference_from_ancestor = glam::Affine3A::IDENTITY;
        while let Some(parent_path) = current_tree.path.parent() {
            let Some(parent_tree) = entity_tree.subtree(&parent_path) else {
                // Unlike not having the space path in the hierarchy, this should be impossible.
                re_log::error_once!(
                    "Path {parent_path} is not part of the global entity tree whereas its child is"
                );
                return;
            };

            // Note that the transform at the reference is the first that needs to be inverted to "break out" of its hierarchy.
            // Generally, the transform _at_ a node isn't relevant to it's children, but only to get to its parent in turn!
            let transforms_at_entity = transforms_at(
                &current_tree.path,
                time_query,
                // TODO(#1025): See comment in transform_at. This is a workaround for precision issues
                // and the fact that there is no meaningful image plane distance for 3D->2D views.
                |_| 500.0,
                &mut None, // Don't care about pinhole encounters.
                transforms,
            );
            let new_transform = transform_info_for_upward_propagation(
                reference_from_ancestor,
                &transforms_at_entity,
            );

            reference_from_ancestor = new_transform.reference_from_entity;

            // (this skips over everything at and under `current_tree` automatically)
            self.gather_descendants_transforms(
                ctx,
                data_result_tree,
                parent_tree,
                time_query,
                new_transform,
                transforms,
            );

            current_tree = parent_tree;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn gather_descendants_transforms(
        &mut self,
        ctx: &ViewContext<'_>,
        data_result_tree: &DataResultTree,
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

            let lookup_image_plane =
                |p: &_| lookup_image_plane_distance(ctx, data_result_tree, p, query);

            let mut encountered_pinhole = twod_in_threed_info
                .as_ref()
                .map(|info| info.parent_pinhole.clone());

            let transforms_at_entity = transforms_at(
                child_path,
                query,
                lookup_image_plane,
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
                ctx,
                data_result_tree,
                child_tree,
                query,
                new_transform,
                transforms_for_timeline,
            );
        }
    }

    pub fn reference_path(&self) -> &EntityPath {
        &self.space_origin
    }

    /// Retrieves transform information for a given entity.
    ///
    /// Returns `None` if it's not reachable from the view's origin.
    pub fn transform_info_for_entity(&self, ent_path: EntityPathHash) -> Option<&TransformInfo> {
        self.transform_per_entity.get(&ent_path)
    }
}

fn lookup_image_plane_distance(
    ctx: &ViewContext<'_>,
    data_result_tree: &DataResultTree,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> f32 {
    data_result_tree
        .lookup_result_by_path(entity_path)
        .cloned()
        .map(|data_result| {
            data_result
                .latest_at_with_blueprint_resolved_data_for_component(
                    ctx,
                    query,
                    &archetypes::Pinhole::descriptor_image_plane_distance(),
                )
                .get_mono_with_fallback::<ImagePlaneDistance>(
                    &archetypes::Pinhole::descriptor_image_plane_distance(),
                )
        })
        .unwrap_or_default()
        .into()
}

/// Compute transform info for when we walk up the tree from the reference.
fn transform_info_for_upward_propagation(
    reference_from_ancestor: glam::Affine3A,
    transforms_at_entity: &TransformsAtEntity<'_>,
) -> TransformInfo {
    let mut reference_from_entity = reference_from_ancestor;

    // Need to take care of the fact that we're walking the other direction of the tree here compared to `transform_info_for_downward_propagation`!
    // Apply inverse transforms in flipped order!

    // Apply 2D->3D transform if present.
    if let Some(entity_from_2d_pinhole_content) =
        transforms_at_entity.instance_from_pinhole_image_plane
    {
        // If we're going up the tree and encounter a pinhole, we still to apply it.
        // This is what handles "3D in 2D".
        reference_from_entity *= entity_from_2d_pinhole_content.inverse();
    }

    // Collect & compute poses.
    let (mut reference_from_instances, has_instance_transforms) =
        if let Ok(mut entity_from_instances) = SmallVec1::<[glam::Affine3A; 1]>::try_from_slice(
            transforms_at_entity.entity_from_instance_poses,
        ) {
            for entity_from_instance in &mut entity_from_instances {
                *entity_from_instance = reference_from_entity * entity_from_instance.inverse();
                // Now this is actually `reference_from_instance`.
            }
            (entity_from_instances, true)
        } else {
            (SmallVec1::new(reference_from_entity), false)
        };

    // Apply tree transform.
    reference_from_entity *= transforms_at_entity
        .parent_from_entity_tree_transform
        .inverse();
    if has_instance_transforms {
        for reference_from_instance in &mut reference_from_instances {
            *reference_from_instance = reference_from_entity * (*reference_from_instance);
        }
    } else {
        *reference_from_instances.first_mut() = reference_from_entity;
    }

    TransformInfo {
        reference_from_entity,
        reference_from_instances,

        // Going up the tree, we can only encounter 2D->3D transforms.
        // 3D->2D transforms can't happen because `Pinhole` represents 3D->2D (and we're walking backwards!)
        twod_in_threed_info: None,
    }
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

    // Collect & compute poses.
    let (mut reference_from_instances, has_instance_transforms) =
        if let Ok(mut entity_from_instances) =
            SmallVec1::try_from_slice(transforms_at_entity.entity_from_instance_poses)
        {
            for entity_from_instance in &mut entity_from_instances {
                *entity_from_instance = reference_from_entity * (*entity_from_instance);
                // Now this is actually `reference_from_instance`.
            }
            (entity_from_instances, true)
        } else {
            (SmallVec1::new(reference_from_entity), false)
        };

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

        // Need to update per instance transforms as well if there are poses!
        if has_instance_transforms {
            *reference_from_instances.first_mut() = reference_from_entity;
        } else {
            for reference_from_instance in &mut reference_from_instances {
                *reference_from_instance *= entity_from_2d_pinhole_content;
            }
        }
    }

    TransformInfo {
        reference_from_entity,
        reference_from_instances,
        twod_in_threed_info,
    }
}

#[cfg(debug_assertions)]
fn debug_assert_transform_field_order(reflection: &re_types::reflection::Reflection) {
    use re_types::{Archetype as _, components};

    let expected_order = vec![
        components::Translation3D::name(),
        components::RotationAxisAngle::name(),
        components::RotationQuat::name(),
        components::Scale3D::name(),
        components::TransformMat3x3::name(),
    ];

    let transform3d_reflection = reflection
        .archetypes
        .get(&re_types::archetypes::Transform3D::name())
        .expect("Transform3D archetype not found in reflection");

    let mut remaining_fields = expected_order.clone();
    for field in transform3d_reflection.fields.iter().rev() {
        if Some(&field.component_name) == remaining_fields.last() {
            remaining_fields.pop();
        }
    }

    if !remaining_fields.is_empty() {
        let actual_order = transform3d_reflection
            .fields
            .iter()
            .map(|f| f.component_name)
            .collect::<Vec<_>>();
        panic!(
            "Expected transform fields in the following order:\n{expected_order:?}\n
But they are instead ordered like this:\n{actual_order:?}"
        );
    }
}

#[cfg(not(debug_assertions))]
fn debug_assert_transform_field_order(_: &re_types::reflection::Reflection) {}

fn transform_from_pinhole_with_image_plane(
    entity_path: &EntityPath,
    resolved_pinhole_projection: &ResolvedPinholeProjection,
    pinhole_image_plane_distance: impl Fn(&EntityPath) -> f32,
) -> glam::Affine3A {
    let ResolvedPinholeProjection {
        image_from_camera,
        view_coordinates,
    } = resolved_pinhole_projection;

    // Everything under a pinhole camera is a 2D projection, thus doesn't actually have a proper 3D representation.
    // Our visualization interprets this as looking at a 2D image plane from a single point (the pinhole).

    // Center the image plane and move it along z, scaling the further the image plane is.
    let distance = pinhole_image_plane_distance(entity_path);
    let focal_length = image_from_camera.focal_length_in_pixels();
    let focal_length = glam::vec2(focal_length.x(), focal_length.y());
    let scale = distance / focal_length;
    let translation = (-image_from_camera.principal_point() * scale).extend(distance);

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
    // However, currently our 2D views require do to exactly that since we're forced to
    // build a relationship between the 2D plane and the 3D world, when actually the 2D plane
    // should have infinite depth!
    // The inverse of this matrix *is* working for this, but quickly runs into precision issues.
    // See also `ui_2d.rs#setup_target_config`
}

/// Resolved transforms at an entity.
#[derive(Default)]
struct TransformsAtEntity<'a> {
    parent_from_entity_tree_transform: glam::Affine3A,
    entity_from_instance_poses: &'a [glam::Affine3A],
    instance_from_pinhole_image_plane: Option<glam::Affine3A>,
}

fn transforms_at<'a>(
    entity_path: &EntityPath,
    query: &LatestAtQuery,
    pinhole_image_plane_distance: impl Fn(&EntityPath) -> f32,
    encountered_pinhole: &mut Option<EntityPath>,
    transforms_for_timeline: &'a CachedTransformsForTimeline,
) -> TransformsAtEntity<'a> {
    // This is called very frequently, don't put a profile scope here.

    let Some(entity_transforms) = transforms_for_timeline.entity_transforms(entity_path) else {
        return TransformsAtEntity::default();
    };

    let parent_from_entity_tree_transform = entity_transforms.latest_at_tree_transform(query);
    let entity_from_instance_poses = entity_transforms.latest_at_instance_poses(query);
    let instance_from_pinhole_image_plane =
        entity_transforms
            .latest_at_pinhole(query)
            .map(|resolved_pinhole_projection| {
                transform_from_pinhole_with_image_plane(
                    entity_path,
                    resolved_pinhole_projection,
                    pinhole_image_plane_distance,
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
