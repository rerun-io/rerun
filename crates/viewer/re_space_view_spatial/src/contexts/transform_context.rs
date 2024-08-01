use nohash_hasher::IntMap;

use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityDb, EntityPath, EntityTree};
use re_space_view::DataResultQuery2 as _;
use re_types::{
    archetypes::Pinhole,
    components::{
        DisconnectedSpace, ImagePlaneDistance, PinholeProjection, RotationAxisAngle, RotationQuat,
        Scale3D, Transform3D, TransformMat3x3, TransformRelation, Translation3D, ViewCoordinates,
    },
    ComponentNameSet, Loggable as _,
};
use re_viewer_context::{IdentifiedViewSystem, ViewContext, ViewContextSystem};

use crate::visualizers::image_view_coordinates;

#[derive(Clone)]
struct TransformInfo {
    /// The transform from the entity to the reference space.
    pub reference_from_entity: glam::Affine3A,

    /// The pinhole camera ancestor of this entity if any.
    ///
    /// None indicates that this entity is under the eye camera with no Pinhole camera in-between.
    /// Some indicates that the entity is under a pinhole camera at the given entity path that is not at the root of the space view.
    pub parent_pinhole: Option<EntityPath>,
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

        let entity_tree = ctx.recording().tree();

        self.space_origin = query.space_origin.clone();

        // Find the entity path tree for the root.
        let Some(mut current_tree) = &entity_tree.subtree(query.space_origin) else {
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
            glam::Affine3A::IDENTITY,
            &None, // Ignore potential pinhole camera at the root of the space view, since it regarded as being "above" this root.
        );

        // Walk up from the reference to the highest reachable parent.
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
            match transform_at(
                current_tree,
                ctx.recording(),
                &time_query,
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
                Ok(None) => {}
                Ok(Some(parent_from_child)) => {
                    reference_from_ancestor *= parent_from_child.inverse();
                }
            }

            // (skip over everything at and under `current_tree` automatically)
            self.gather_descendants_transforms(
                ctx,
                query,
                parent_tree,
                ctx.recording(),
                &time_query,
                reference_from_ancestor,
                &encountered_pinhole,
            );

            current_tree = parent_tree;
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl TransformContext {
    #[allow(clippy::too_many_arguments)]
    fn gather_descendants_transforms(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &re_viewer_context::ViewQuery<'_>,
        subtree: &EntityTree,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
        reference_from_entity: glam::Affine3A,
        encountered_pinhole: &Option<EntityPath>,
    ) {
        match self.transform_per_entity.entry(subtree.path.clone()) {
            std::collections::hash_map::Entry::Occupied(_) => {
                return;
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(TransformInfo {
                    reference_from_entity,
                    parent_pinhole: encountered_pinhole.clone(),
                });
            }
        }

        for child_tree in subtree.children.values() {
            let mut encountered_pinhole = encountered_pinhole.clone();

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

            let reference_from_child = match transform_at(
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
                Ok(None) => reference_from_entity,
                Ok(Some(child_from_parent)) => reference_from_entity * child_from_parent,
            };
            self.gather_descendants_transforms(
                ctx,
                view_query,
                child_tree,
                entity_db,
                query,
                reference_from_child,
                &encountered_pinhole,
            );
        }
    }

    pub fn reference_path(&self) -> &EntityPath {
        &self.space_origin
    }

    /// Retrieves the transform of on entity from its local system to the space of the reference.
    ///
    /// Returns None if the path is not reachable.
    pub fn reference_from_entity(&self, ent_path: &EntityPath) -> Option<glam::Affine3A> {
        self.transform_per_entity
            .get(ent_path)
            .map(|i| i.reference_from_entity)
    }

    /// Like [`Self::reference_from_entity`], but if `ent_path` has a pinhole camera, it won't affect the transform.
    ///
    /// Normally, the transform we compute for an entity with a pinhole transform places all objects
    /// in front (defined by view coordinates) of the camera with a given image plane distance.
    /// In some cases like drawing the lines for a frustum or arrows for the 3D transform, this is not the desired transformation.
    /// Returns None if the path is not reachable.
    ///
    /// TODO(#2663, #1025): Going forward we should have separate transform hierarchies for 2D (i.e. projected) and 3D,
    /// which would remove the need for this.
    pub fn reference_from_entity_ignoring_pinhole(
        &self,
        ent_path: &EntityPath,
        entity_db: &EntityDb,
        query: &LatestAtQuery,
    ) -> Option<glam::Affine3A> {
        let transform_info = self.transform_per_entity.get(ent_path)?;
        if let (true, Some(parent)) = (
            transform_info.parent_pinhole.as_ref() == Some(ent_path),
            ent_path.parent(),
        ) {
            self.reference_from_entity(&parent).map(|t| {
                t * get_parent_from_child_transform(ent_path, entity_db, query).unwrap_or_default()
            })
        } else {
            Some(transform_info.reference_from_entity)
        }
    }

    /// Retrieves the ancestor (or self) pinhole under which this entity sits.
    ///
    /// None indicates either that the entity does not exist in this hierarchy or that this entity is under the eye camera with no Pinhole camera in-between.
    /// Some indicates that the entity is under a pinhole camera at the given entity path that is not at the root of the space view.
    pub fn parent_pinhole(&self, ent_path: &EntityPath) -> Option<&EntityPath> {
        self.transform_per_entity
            .get(ent_path)
            .and_then(|i| i.parent_pinhole.as_ref())
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

fn get_parent_from_child_transform(
    entity_path: &EntityPath,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
) -> Option<glam::Affine3A> {
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
        ],
    );
    if result.components.is_empty() {
        return None;
    }

    // Order is specified by order of components in the Transform3D archetype.
    // See `has_transform_expected_order`
    let mut transform = glam::Affine3A::IDENTITY;
    if let Some(translation) = result.component_instance::<Translation3D>(0) {
        transform *= glam::Affine3A::from(translation);
    }
    if let Some(rotation) = result.component_instance::<RotationAxisAngle>(0) {
        transform *= glam::Affine3A::from(rotation);
    }
    if let Some(rotation) = result.component_instance::<RotationQuat>(0) {
        transform *= glam::Affine3A::from(rotation);
    }
    if let Some(scale) = result.component_instance::<Scale3D>(0) {
        transform *= glam::Affine3A::from(scale);
    }
    if let Some(mat3x3) = result.component_instance::<TransformMat3x3>(0) {
        transform *= glam::Affine3A::from(mat3x3);
    }

    let transform_relation = result
        .component_instance::<TransformRelation>(0)
        .unwrap_or_default();
    if transform_relation == TransformRelation::ChildFromParent {
        Some(transform.inverse())
    } else {
        Some(transform)
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
        .map(|(_index, image_from_camera)| {
            (
                image_from_camera,
                entity_db
                    .latest_at_component::<ViewCoordinates>(entity_path, query)
                    .map_or(ViewCoordinates::RDF, |(_index, res)| res),
            )
        })
}

fn transform_at(
    subtree: &EntityTree,
    entity_db: &EntityDb,
    query: &LatestAtQuery,
    pinhole_image_plane_distance: impl Fn(&EntityPath) -> f32,
    encountered_pinhole: &mut Option<EntityPath>,
) -> Result<Option<glam::Affine3A>, UnreachableTransformReason> {
    re_tracing::profile_function!();

    let entity_path = &subtree.path;

    let pinhole = get_cached_pinhole(entity_path, entity_db, query);
    if pinhole.is_some() {
        if encountered_pinhole.is_some() {
            return Err(UnreachableTransformReason::NestedPinholeCameras);
        } else {
            *encountered_pinhole = Some(entity_path.clone());
        }
    }

    // If this entity does not contain any `Transform3D`-related data at all, there's no
    // point in running actual queries.
    let is_potentially_transformed =
        crate::transform_component_tracker::TransformComponentTracker::access(
            entity_db.store_id(),
            |transform_component_tracker| {
                transform_component_tracker.is_potentially_transformed(entity_path)
            },
        )
        .unwrap_or(false);
    let transform3d = is_potentially_transformed
        .then(|| get_parent_from_child_transform(entity_path, entity_db, query))
        .flatten();

    let pinhole = pinhole.map(|(image_from_camera, camera_xyz)| {
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
        let world_from_image_plane3d = camera_xyz.from_other(&image_view_coordinates());

        glam::Affine3A::from_mat3(world_from_image_plane3d) * image_plane3d_from_2d_content

        // Above calculation is nice for a certain kind of visualizing a projected image plane,
        // but the image plane distance is arbitrary and there might be other, better visualizations!

        // TODO(#1025):
        // As such we don't ever want to invert this matrix!
        // However, currently our 2D views require do to exactly that since we're forced to
        // build a relationship between the 2D plane and the 3D world, when actually the 2D plane
        // should have infinite depth!
        // The inverse of this matrix *is* working for this, but quickly runs into precision issues.
        // See also `ui_2d.rs#setup_target_config`
    });

    let is_disconnect_space = || {
        entity_db
            .latest_at_component::<DisconnectedSpace>(entity_path, query)
            .map_or(false, |(_index, res)| **res)
    };

    // If there is any other transform, we ignore `DisconnectedSpace`.
    if transform3d.is_some() || pinhole.is_some() {
        Ok(Some(
            transform3d.unwrap_or(glam::Affine3A::IDENTITY)
                * pinhole.unwrap_or(glam::Affine3A::IDENTITY),
        ))
    } else if is_disconnect_space() {
        Err(UnreachableTransformReason::DisconnectedSpace)
    } else {
        Ok(None)
    }
}
