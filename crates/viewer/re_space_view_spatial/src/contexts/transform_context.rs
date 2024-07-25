use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityDb, EntityPath, EntityTree};
use re_space_view::DataResultQuery as _;
use re_types::{
    archetypes::Pinhole,
    components::{
        DisconnectedSpace, ImagePlaneDistance, OutOfTreeTransform, PinholeProjection,
        RotationAxisAngle, RotationQuat, Scale3D, Transform3D, TransformMat3x3, TransformRelation,
        Translation3D, ViewCoordinates,
    },
    ComponentNameSet, Loggable as _,
};
use re_viewer_context::{IdentifiedViewSystem, ViewContext, ViewContextSystem};

use crate::visualizers::{entity_iterator::clamped_or_nothing, image_view_coordinates};

#[derive(Clone, Debug)]
pub struct TransformInfo {
    /// The transform from the entity to the reference space.
    ///
    /// Does not include out-of-tree transforms!
    pub reference_from_entity: glam::Affine3A,

    /// Like [`TransformInfo::reference_from_entity`], but if this entity has a pinhole camera, it won't affect the transform.
    ///
    /// Normally, the transform we compute for an entity with a pinhole transform places all objects
    /// in front (defined by view coordinates) of the camera with a given image plane distance.
    /// In some cases like drawing the lines for a frustum or arrows for the 3D transform, this is not the desired transformation.
    ///
    /// TODO(andreas): This a lot of overhead for when we don't need it (which is most of the time!).
    ///
    /// TODO(#2663, #1025): Going forward we should have separate transform hierarchies for 2D (i.e. projected) and 3D,
    /// which would remove the need for this.
    pub reference_from_entity_ignoring_3d_from_2d_pinhole: glam::Affine3A,

    /// Optional list of out of tree transforms that are applied to the instances of this entity.
    pub out_of_tree_transforms: Vec<glam::Affine3A>,

    /// The pinhole camera ancestor of this entity if any.
    ///
    /// None indicates that this entity is under the eye camera with no Pinhole camera in-between.
    /// Some indicates that the entity is under a pinhole camera at the given entity path that is not at the root of the space view.
    pub parent_pinhole: Option<EntityPath>,
}

impl Default for TransformInfo {
    fn default() -> Self {
        Self {
            reference_from_entity: glam::Affine3A::IDENTITY,
            reference_from_entity_ignoring_3d_from_2d_pinhole: glam::Affine3A::IDENTITY,
            out_of_tree_transforms: Vec::new(),
            parent_pinhole: None,
        }
    }
}

#[derive(Clone, Copy)]
enum UnreachableTransformReason {
    /// More than one pinhole camera between this and the reference space.
    NestedPinholeCameras,

    /// Unknown transform between this and the reference space.
    DisconnectedSpace,
}

/// Provides transforms from an entity to a chosen reference space for all elements in the scene
/// for the currently selected time & timeline.
///
/// The renderer then uses this reference space as its world space,
/// making world and reference space equivalent for a given space view.
///
/// Should be recomputed every frame.
#[derive(Clone)]
pub struct TransformContext {
    /// All transforms provided are relative to this reference path.
    space_origin: EntityPath,

    /// All reachable entities.
    transform_per_entity: IntMap<EntityPath, TransformInfo>,

    /// All unreachable descendant paths of `reference_path`.
    unreachable_descendants: Vec<(EntityPath, UnreachableTransformReason)>,

    /// The first parent of reference_path that is no longer reachable.
    first_unreachable_parent: Option<(EntityPath, UnreachableTransformReason)>,
}

impl IdentifiedViewSystem for TransformContext {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "TransformContext".into()
    }
}

impl Default for TransformContext {
    fn default() -> Self {
        Self {
            space_origin: EntityPath::root(),
            transform_per_entity: Default::default(),
            unreachable_descendants: Default::default(),
            first_unreachable_parent: None,
        }
    }
}

impl ViewContextSystem for TransformContext {
    fn compatible_component_sets(&self) -> Vec<ComponentNameSet> {
        vec![
            std::iter::once(Transform3D::name()).collect(),
            std::iter::once(PinholeProjection::name()).collect(),
            std::iter::once(DisconnectedSpace::name()).collect(),
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

        debug_assert_transform_field_order(ctx.viewer_ctx.reflection);

        self.space_origin = query.space_origin.clone();

        // Find the entity path tree for the root.
        let Some(current_tree) = &ctx.recording().tree().subtree(query.space_origin) else {
            // It seems the space path is not part of the object tree!
            // This happens frequently when the viewer remembers space views from a previous run that weren't shown yet.
            // Naturally, in this case we don't have any transforms yet.
            return;
        };

        let time_query = ctx.current_query();

        // Child transforms of this space
        self.gather_descendants_transforms(
            ctx,
            query,
            current_tree,
            ctx.recording(),
            &time_query,
            // Ignore potential pinhole camera at the root of the space view, since it regarded as being "above" this root.
            TransformInfo::default(),
        );

        // Walk up from the reference to the highest reachable parent.
        self.gather_parent_transforms(ctx, query, current_tree, &time_query);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TransformContext {
    /// Gather transforms for everything _above_ the root.
    fn gather_parent_transforms<'a>(
        &mut self,
        ctx: &'a ViewContext<'a>,
        query: &re_viewer_context::ViewQuery<'_>,
        mut current_tree: &'a EntityTree,
        time_query: &LatestAtQuery,
    ) {
        re_tracing::profile_function!();

        let entity_tree = ctx.recording().tree();

        let mut encountered_pinhole = None;
        let mut reference_from_ancestor = glam::Affine3A::IDENTITY;
        while let Some(parent_path) = current_tree.path.parent() {
            let Some(parent_tree) = entity_tree.subtree(&parent_path) else {
                // Unlike not having the space path in the hierarchy, this should be impossible.
                re_log::error_once!(
                    "Path {} is not part of the global entity tree whereas its child {} is",
                    parent_path,
                    query.space_origin
                );
                return;
            };

            // Note that the transform at the reference is the first that needs to be inverted to "break out" of its hierarchy.
            // Generally, the transform _at_ a node isn't relevant to it's children, but only to get to its parent in turn!
            let new_transform = match transform_at(
                current_tree,
                ctx.recording(),
                time_query,
                // TODO(#1025): See comment in transform_at. This is a workaround for precision issues
                // and the fact that there is no meaningful image plane distance for 3D->2D views.
                |_| 500.0,
                &mut encountered_pinhole,
            ) {
                Err(unreachable_reason) => {
                    self.first_unreachable_parent =
                        Some((parent_tree.path.clone(), unreachable_reason));
                    break;
                }
                Ok((transform_at_entity, entity_from_2d_pinhole_content)) => {
                    let mut new_transform = TransformInfo {
                        reference_from_entity: reference_from_ancestor,
                        reference_from_entity_ignoring_3d_from_2d_pinhole: reference_from_ancestor,
                        out_of_tree_transforms: Vec::new(),
                        parent_pinhole: encountered_pinhole.clone(),
                    };

                    // Need to take care of the fact that we're walking the other direction of the tree here compared to `gather_descendants_transforms`!
                    if let Some(entity_from_2d_pinhole_content) = entity_from_2d_pinhole_content {
                        debug_assert!(encountered_pinhole.as_ref() == Some(&current_tree.path));
                        new_transform.reference_from_entity *=
                            entity_from_2d_pinhole_content.0.inverse();
                    }

                    match transform_at_entity {
                        TransformsAtEntity::None => {}
                        TransformsAtEntity::TreeTransform(child_from_entity) => {
                            let entity_from_child = child_from_entity.inverse();
                            new_transform.reference_from_entity *= entity_from_child;
                            new_transform.reference_from_entity_ignoring_3d_from_2d_pinhole *=
                                entity_from_child;
                        }
                        TransformsAtEntity::OutOfTreeTransforms(out_of_tree_transforms) => {
                            new_transform.out_of_tree_transforms = out_of_tree_transforms
                                .iter()
                                .map(|&t| t.inverse())
                                .collect();
                        }
                    }

                    // If we're going up the tree and encounter a pinhole, we need to apply it, it means that our reference is 2D.
                    // So it's not acutally a "3d_from_2d" but rather a "2d_from_3d", consequently
                    // `reference_from_entity_ignoring_3d_from_2d_pinhole`is always the same as `reference_from_entity`.
                    new_transform.reference_from_entity_ignoring_3d_from_2d_pinhole =
                        new_transform.reference_from_entity;

                    new_transform
                }
            };

            reference_from_ancestor = new_transform.reference_from_entity;

            // (this skips over everything at and under `current_tree` automatically)
            self.gather_descendants_transforms(
                ctx,
                query,
                parent_tree,
                ctx.recording(),
                time_query,
                new_transform,
            );

            current_tree = parent_tree;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn gather_descendants_transforms(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &re_viewer_context::ViewQuery<'_>,
        subtree: &EntityTree,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
        transform: TransformInfo,
    ) {
        let encountered_pinhole = transform.parent_pinhole.clone();
        let reference_from_parent = transform.reference_from_entity;
        let reference_from_parent_ignoring_3d_from_2d_pinhole =
            transform.reference_from_entity_ignoring_3d_from_2d_pinhole;
        match self.transform_per_entity.entry(subtree.path.clone()) {
            std::collections::hash_map::Entry::Occupied(_) => {
                return;
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(transform);
            }
        }

        for child_tree in subtree.children.values() {
            let lookup_image_plane = |p: &_| {
                let query_result = ctx.viewer_ctx.lookup_query_result(view_query.space_view_id);

                query_result
                    .tree
                    .lookup_result_by_path(p)
                    .cloned()
                    .map(|data_result| {
                        let results = data_result
                            .latest_at_with_blueprint_resolved_data::<Pinhole>(ctx, query);

                        results.get_mono_with_fallback::<ImagePlaneDistance>()
                    })
                    .unwrap_or_default()
                    .into()
            };

            let mut encountered_pinhole = encountered_pinhole.clone();
            let new_transform = match transform_at(
                child_tree,
                entity_db,
                query,
                lookup_image_plane,
                &mut encountered_pinhole,
            ) {
                Err(unreachable_reason) => {
                    self.unreachable_descendants
                        .push((child_tree.path.clone(), unreachable_reason));
                    continue;
                }

                Ok((transform_at_entity, entity_from_2d_pinhole_content)) => {
                    let mut new_transform = TransformInfo {
                        reference_from_entity: reference_from_parent,
                        reference_from_entity_ignoring_3d_from_2d_pinhole:
                            reference_from_parent_ignoring_3d_from_2d_pinhole,
                        out_of_tree_transforms: Vec::new(),
                        parent_pinhole: encountered_pinhole.clone(),
                    };

                    match transform_at_entity {
                        TransformsAtEntity::None => {}
                        TransformsAtEntity::TreeTransform(parent_from_entity) => {
                            new_transform.reference_from_entity *= parent_from_entity;
                            new_transform.reference_from_entity_ignoring_3d_from_2d_pinhole *=
                                parent_from_entity;
                        }
                        TransformsAtEntity::OutOfTreeTransforms(out_of_tree_transforms) => {
                            new_transform.out_of_tree_transforms = out_of_tree_transforms;
                        }
                    }

                    if let Some(entity_from_2d_pinhole_content) = entity_from_2d_pinhole_content {
                        debug_assert!(encountered_pinhole.as_ref() == Some(&child_tree.path));
                        new_transform.reference_from_entity *= entity_from_2d_pinhole_content.0;
                    }

                    new_transform
                }
            };

            self.gather_descendants_transforms(
                ctx,
                view_query,
                child_tree,
                entity_db,
                query,
                new_transform,
            );
        }
    }

    pub fn reference_path(&self) -> &EntityPath {
        &self.space_origin
    }

    /// Retrives transform information for a given entity.
    ///
    /// Returns `None` if it's not reachable from the view's origin.
    pub fn transform_info_for_entity(&self, ent_path: &EntityPath) -> Option<&TransformInfo> {
        self.transform_per_entity.get(ent_path)
    }
}

#[cfg(debug_assertions)]
fn debug_assert_transform_field_order(reflection: &re_types::reflection::Reflection) {
    let expected_order = vec![
        Translation3D::name(),
        RotationAxisAngle::name(),
        RotationQuat::name(),
        Scale3D::name(),
        TransformMat3x3::name(),
    ];

    use re_types::Archetype as _;
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

fn assemble_transform(
    translation: Option<&Translation3D>,
    rotation_quat: Option<&RotationQuat>,
    rotation_axis_angle: Option<&RotationAxisAngle>,
    scale: Option<&Scale3D>,
    mat3x3: Option<&TransformMat3x3>,
    transform_relation: TransformRelation,
) -> glam::Affine3A {
    // Order see `debug_assert_transform_field_order`
    let mut transform = glam::Affine3A::IDENTITY;

    if let Some(translation) = translation {
        transform = glam::Affine3A::from(*translation);
    }
    if let Some(rotation_quat) = rotation_quat {
        transform *= glam::Affine3A::from(*rotation_quat);
    }
    if let Some(rotation_axis_angle) = rotation_axis_angle {
        transform *= glam::Affine3A::from(*rotation_axis_angle);
    }
    if let Some(scale) = scale {
        transform *= glam::Affine3A::from(*scale);
    }
    if let Some(mat3x3) = mat3x3 {
        transform *= glam::Affine3A::from(*mat3x3);
    }

    if transform_relation == TransformRelation::ChildFromParent {
        transform = transform.inverse();
    }

    transform
}

/// Utility representing the possible resolved transforms at an entity.
enum TransformsAtEntity {
    None,
    TreeTransform(glam::Affine3A),
    OutOfTreeTransforms(Vec<glam::Affine3A>),
}

struct ObjFrom2DPinholeContent(glam::Affine3A);

impl TransformsAtEntity {
    fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Utility method to implement fallback provider fo `OutOfTreeTransform`.
pub fn fallback_for_out_of_tree_transform(ctx: &re_viewer_context::QueryContext<'_>) -> bool {
    let result = ctx.recording().latest_at(
        ctx.query,
        ctx.target_entity_path,
        [
            Transform3D::name(),
            Translation3D::name(),
            RotationAxisAngle::name(),
            RotationQuat::name(),
            Scale3D::name(),
            TransformMat3x3::name(),
        ],
    );
    if result.components.is_empty() {
        return false;
    }

    result
        .components
        .values()
        .map(|result| result.num_instances())
        .max()
        .unwrap_or(0)
        > 1
}

fn query_and_resolve_transforms_at_entity(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> TransformsAtEntity {
    let resolver = entity_db.resolver();
    // TODO(#6743): Doesn't take into account overrides.
    let result = entity_db.latest_at(
        query,
        entity_path,
        [
            Transform3D::name(),
            Translation3D::name(),
            RotationAxisAngle::name(),
            RotationQuat::name(),
            Scale3D::name(),
            TransformMat3x3::name(),
            TransformRelation::name(),
            OutOfTreeTransform::name(),
        ],
    );
    if result.components.is_empty() {
        return TransformsAtEntity::None;
    }

    let translations = result.get_slice::<Translation3D>(resolver).unwrap_or(&[]);
    let scales = result.get_slice::<Scale3D>(resolver).unwrap_or(&[]);
    let rotation_quats = result.get_slice::<RotationQuat>(resolver).unwrap_or(&[]);
    let rotation_axis_angles = result
        .get_slice::<RotationAxisAngle>(resolver)
        .unwrap_or(&[]);
    let mat3x3 = result.get_slice::<TransformMat3x3>(resolver).unwrap_or(&[]);

    let max_count = translations
        .len()
        .max(scales.len())
        .max(rotation_quats.len())
        .max(rotation_axis_angles.len())
        .max(mat3x3.len());

    if max_count == 0 {
        return TransformsAtEntity::None;
    }

    // Default out of tree transform to true if any of the transform components are set more than once.
    let out_of_tree = result
        .get_instance::<OutOfTreeTransform>(resolver, 0)
        .map_or(max_count > 1, |c| *c.0);

    if !out_of_tree && max_count > 1 {
        re_log::error!(
            "Entity {:?} has multiple instances of transform components, but OutOfTreeTransform is set to false.
Propagating multiple transforms to children is not supported",
            entity_path
        );
        return TransformsAtEntity::None;
    }

    let transform_relation = result
        .get_instance::<TransformRelation>(resolver, 0)
        .unwrap_or_default();

    if out_of_tree {
        // Order is specified by order of components in the Transform3D archetype.
        // See `has_transform_expected_order`

        let mut out_of_tree_transforms = Vec::with_capacity(max_count);

        let mut iter_scales = clamped_or_nothing(scales, max_count);
        let mut iter_rotation_quats = clamped_or_nothing(rotation_quats, max_count);
        let mut iter_rotation_axis_angles = clamped_or_nothing(rotation_axis_angles, max_count);
        let mut iter_translations = clamped_or_nothing(translations, max_count);
        let mut iter_mat3x3 = clamped_or_nothing(mat3x3, max_count);

        for _ in 0..max_count {
            out_of_tree_transforms.push(assemble_transform(
                iter_translations.next(),
                iter_rotation_quats.next(),
                iter_rotation_axis_angles.next(),
                iter_scales.next(),
                iter_mat3x3.next(),
                transform_relation,
            ));
        }

        TransformsAtEntity::OutOfTreeTransforms(out_of_tree_transforms)
    } else {
        // Fast path for no out of tree transforms.
        TransformsAtEntity::TreeTransform(assemble_transform(
            translations.first(),
            rotation_quats.first(),
            rotation_axis_angles.first(),
            scales.first(),
            mat3x3.first(),
            transform_relation,
        ))
    }

    // TODO(#6831): Should add a unit test to this method once all variants are in.
    // (Should test correct order being applied etc.. Might require splitting)
}

fn get_cached_pinhole(
    entity_path: &re_log_types::EntityPath,
    entity_db: &EntityDb,
    query: &re_chunk_store::LatestAtQuery,
) -> Option<(PinholeProjection, ViewCoordinates)> {
    entity_db
        .latest_at_component::<PinholeProjection>(entity_path, query)
        .map(|image_from_camera| {
            (
                image_from_camera.value,
                entity_db
                    .latest_at_component::<ViewCoordinates>(entity_path, query)
                    .map_or(ViewCoordinates::RDF, |res| res.value),
            )
        })
}

fn transform_at(
    subtree: &EntityTree,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
    pinhole_image_plane_distance: impl Fn(&EntityPath) -> f32,
    encountered_pinhole: &mut Option<EntityPath>,
) -> Result<(TransformsAtEntity, Option<ObjFrom2DPinholeContent>), UnreachableTransformReason> {
    re_tracing::profile_function!();

    let entity_path = &subtree.path;
    let transforms_at_entity =
        query_and_resolve_transforms_at_entity(entity_path, entity_db, query);

    // Special pinhole handling.
    // TODO(#1025): We should start a proper 2D subspace here instead of making up a 2D -> 3D transform.
    let pinhole_transform = if let Some((image_from_camera, camera_xyz)) =
        get_cached_pinhole(entity_path, entity_db, query)
    {
        if encountered_pinhole.is_some() {
            return Err(UnreachableTransformReason::NestedPinholeCameras);
        } else {
            *encountered_pinhole = Some(entity_path.clone());
        }

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
        let obj_from_image_plane3d = camera_xyz.from_other(&image_view_coordinates());

        Some(ObjFrom2DPinholeContent(
            glam::Affine3A::from_mat3(obj_from_image_plane3d) * image_plane3d_from_2d_content,
        ))

        // Above calculation is nice for a certain kind of visualizing a projected image plane,
        // but the image plane distance is arbitrary and there might be other, better visualizations!

        // TODO(#1025):
        // As such we don't ever want to invert this matrix!
        // However, currently our 2D views require do to exactly that since we're forced to
        // build a relationship between the 2D plane and the 3D world, when actually the 2D plane
        // should have infinite depth!
        // The inverse of this matrix *is* working for this, but quickly runs into precision issues.
        // See also `ui_2d.rs#setup_target_config`
    } else {
        None
    };

    // If there is any other transform, we ignore `DisconnectedSpace`.
    if transforms_at_entity.is_none()
        && pinhole_transform.is_none()
        && entity_db
            .latest_at_component::<DisconnectedSpace>(entity_path, query)
            .map_or(false, |res| **res.value)
    {
        Err(UnreachableTransformReason::DisconnectedSpace)
    } else {
        Ok((transforms_at_entity, pinhole_transform))
    }
}
