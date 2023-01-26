use nohash_hasher::IntMap;
use re_arrow_store::Timeline;
use re_data_store::{log_db::ObjDb, query_transform, ObjPath, ObjectTree, ObjectsProperties};

use crate::misc::TimeControl;

/// Provides transforms from an object to a chosen reference space for all elements in the scene.
///
/// The renderer then uses this reference space as its world space,
/// making world and reference space equivalent for a given space view.
#[derive(Clone)]
pub struct TransformCache {
    /// All transforms provided are relative to this reference path.
    #[allow(dead_code)]
    reference_path: ObjPath,

    /// Alll reachable objects.
    reference_from_obj_per_object: IntMap<ObjPath, glam::Mat4>,

    /// All unreachable parents.
    unreachable_paths: Vec<(ObjPath, UnreachableTransformReason)>,

    /// The first parent of reference_path that is no longer reachable.
    first_unreachable_parent: Option<(ObjPath, UnreachableTransformReason)>,
}

#[derive(Clone, Copy)]
pub enum UnreachableTransformReason {
    /// Not part of the hierarchy at all.
    Unconnected,
    /// More than one pinhole camera between this and the reference space.
    NestedPinholeCameras,
    /// Exiting out of a space with a pinhole camera that doesn't have a resolution is not supported.
    InversePinholeCameraWithoutResolution,
    /// Unknown transform between this and the reference space.
    UnknownTransform,
}

impl TransformCache {
    /// Determines transforms for all objects relative to a root path which serves as the "reference".
    /// I.e. the resulting transforms are "reference from scene"
    ///
    /// This means that the objects in `reference_space` get the identity transform and all other
    /// objects are transformed relative to it.
    pub fn determine_transforms(
        obj_db: &ObjDb,
        time_ctrl: &TimeControl,
        root_path: &ObjPath,
        obj_properties: &ObjectsProperties,
    ) -> Self {
        crate::profile_function!();

        let mut transforms = TransformCache {
            reference_path: root_path.clone(),
            reference_from_obj_per_object: Default::default(),
            unreachable_paths: Default::default(),
            first_unreachable_parent: None,
        };

        // Find the object path tree for the root.
        let mut parent_tree_stack = Vec::new();
        let mut tree_at_root_path = &obj_db.tree;
        'outer: while &tree_at_root_path.path != root_path {
            for child_tree in tree_at_root_path.children.values() {
                if root_path == &child_tree.path || root_path.is_descendant_of(&child_tree.path) {
                    parent_tree_stack.push(tree_at_root_path);
                    tree_at_root_path = child_tree;
                    continue 'outer;
                }
            }
            // Should never reach this
            re_log::warn_once!(
                "Path {} doesn't seem to be part of the global object tree",
                root_path
            );
            return transforms;
        }

        let timeline = time_ctrl.timeline();
        let query_time = time_ctrl.time_i64();

        // Child transforms of this space
        transforms.gather_descendents_transforms(
            tree_at_root_path,
            obj_db,
            timeline,
            query_time,
            obj_properties,
            glam::Mat4::IDENTITY,
            false,
            None,
        );

        // Walk up from the reference to the highest reachable parent.
        let mut encountered_pinhole = false;
        let mut reference_from_ancestor = glam::Mat4::IDENTITY;
        let mut previous_tree = tree_at_root_path;

        while let Some(parent_tree) = parent_tree_stack.pop() {
            let parent_transform = query_transform(obj_db, timeline, &parent_tree.path, query_time);

            if let Some(parent_transform) = parent_transform {
                reference_from_ancestor = match parent_transform {
                    re_log_types::Transform::Rigid3(rigid) => {
                        reference_from_ancestor * rigid.child_from_parent().to_mat4()
                    }
                    // If we're connected via 'unknown', everything except whats under `parent_tree` is unreachable
                    re_log_types::Transform::Unknown => {
                        transforms.first_unreachable_parent = Some((
                            parent_tree.path.clone(),
                            UnreachableTransformReason::UnknownTransform,
                        ));
                        break;
                    }

                    re_log_types::Transform::Pinhole(pinhole) => {
                        if encountered_pinhole {
                            transforms.first_unreachable_parent = Some((
                                parent_tree.path.clone(),
                                UnreachableTransformReason::NestedPinholeCameras,
                            ));
                            break;
                        }
                        encountered_pinhole = true;

                        // TODO(andreas): If we don't have a resolution we don't know the FOV ergo we don't know how to project. Unclear what to do.
                        if let Some(resolution) = pinhole.resolution() {
                            let translation = pinhole.principal_point().extend(-100.0); // Large Y offset so this is in front of all 2d that came so far. TODO(andreas): Find better solution
                            reference_from_ancestor
                                * glam::Mat4::from_scale_rotation_translation(
                                    // Scaled with 0.5 since perspective_infinite_lh uses NDC, i.e. [-1; 1] range.
                                    (resolution * 0.5).extend(1.0),
                                    glam::Quat::IDENTITY,
                                    translation,
                                )
                                * glam::Mat4::perspective_infinite_lh(
                                    pinhole.fov_y().unwrap(),
                                    pinhole.aspect_ratio().unwrap_or(1.0),
                                    0.0,
                                )
                        } else {
                            transforms.first_unreachable_parent = Some((
                                parent_tree.path.clone(),
                                UnreachableTransformReason::InversePinholeCameraWithoutResolution,
                            ));
                            break;
                        }
                    }
                }
            }

            transforms.gather_descendents_transforms(
                tree_at_root_path,
                obj_db,
                timeline,
                query_time,
                obj_properties,
                reference_from_ancestor,
                encountered_pinhole,
                Some(&previous_tree.path),
            );
            previous_tree = parent_tree;
        }

        transforms
    }

    fn gather_descendents_transforms(
        &mut self,
        tree: &ObjectTree,
        obj_db: &ObjDb,
        timeline: &Timeline,
        query_time: Option<i64>,
        obj_properties: &ObjectsProperties,
        reference_from_obj: glam::Mat4,
        encountered_pinhole: bool,
        skipped_child_path: Option<&ObjPath>,
    ) {
        self.reference_from_obj_per_object
            .insert(tree.path.clone(), reference_from_obj);

        for child_tree in tree.children.values() {
            if Some(&child_tree.path) == skipped_child_path {
                continue;
            }
            let child_transform = query_transform(obj_db, timeline, &child_tree.path, query_time);

            let mut encountered_pinhole = encountered_pinhole;
            let reference_from_child = if let Some(child_transform) = child_transform {
                match child_transform {
                    re_log_types::Transform::Rigid3(rigid) => {
                        reference_from_obj * rigid.parent_from_child().to_mat4()
                    }
                    // If we're connected via 'unknown' it's not reachable
                    re_log_types::Transform::Unknown => {
                        self.unreachable_paths.push((
                            child_tree.path.clone(),
                            UnreachableTransformReason::UnknownTransform,
                        ));
                        continue;
                    }

                    re_log_types::Transform::Pinhole(pinhole) => {
                        if encountered_pinhole {
                            self.unreachable_paths.push((
                                child_tree.path.clone(),
                                UnreachableTransformReason::NestedPinholeCameras,
                            ));
                            continue;
                        }
                        encountered_pinhole = true;

                        // A pinhole camera means that we're looking at an image.
                        // Images are spanned in their local x/y space.
                        // Center it and move it along z, scaling the further we move.

                        let distance = obj_properties
                            .get(&child_tree.path)
                            .pinhole_image_plane_distance(&pinhole);

                        let focal_length = pinhole.focal_length_in_pixels();
                        let focal_length = glam::vec2(focal_length.x(), focal_length.y());
                        let scale = distance / focal_length;
                        let translation = (-pinhole.principal_point() * scale).extend(distance);
                        let parent_from_child = glam::Mat4::from_scale_rotation_translation(
                            // We want to preserve any depth that might be on the pinhole image.
                            // Use harmonic mean of x/y scale for those.
                            scale.extend(1.0 / (1.0 / scale.x + 1.0 / scale.y)),
                            glam::Quat::IDENTITY,
                            translation,
                        );

                        reference_from_obj * parent_from_child
                    }
                }
            } else {
                reference_from_obj
            };

            self.gather_descendents_transforms(
                child_tree,
                obj_db,
                timeline,
                query_time,
                obj_properties,
                reference_from_child,
                encountered_pinhole,
                skipped_child_path,
            );
        }
    }

    /// Retrieves the transform of on object from its local system to the space of the reference.
    ///
    /// Only returns None if the path is not reachable. Use [`unreachable_reason`] to determine why.
    pub fn reference_from_obj(&self, obj_path: &ObjPath) -> Option<macaw::Mat4> {
        self.reference_from_obj_per_object.get(obj_path).cloned()
    }

    // This method isn't currently implemented, but we might need it in the future.
    // All the necessary data on why a subtree isn't reachable is already stored.
    //
    // Returns why (if actually) a path isn't reachable.
    // pub fn unreachable_reason(&self, _obj_path: &ObjPath) -> Option<UnreachableTransformReason> {
    //     None
    // }
}
