use nohash_hasher::IntMap;
use re_data_store::{ObjPath, ObjectsProperties};
use re_log_types::ObjPathHash;

use crate::misc::space_info::{SpaceInfo, SpacesInfo};

/// Provides transforms from an object to a chosen reference space for all elements in the scene.
///
/// The renderer then uses this reference space as its world space,
/// making world and reference space equivalent for a given space view.
pub struct TransformCache {
    reference_from_obj_per_object: IntMap<ObjPathHash, ReferenceFromObjTransform>,
}

#[derive(Clone, Copy)]
pub enum UnreachableTransformReason {
    /// Not part of the hierarchy at all.
    Unconnected,
    /// More than one pinhole camera between this and the reference space.
    NestedPinholeCameras,
    /// Unknown transform between this and the reference space.
    UnknownTransform,

    /// Reference space is under pinhole going from this space.
    /// TODO(andreas): Support this one soon!
    ReferenceIsUnderPinhole,
}

#[derive(Clone)]
pub enum ReferenceFromObjTransform {
    /// On the path from the given object to the reference is an obstacle.
    /// TODO(andreas): Can we be more specific to give more information?
    Unreachable(UnreachableTransformReason),

    /// We're able to transform this object into the reference space.
    Reachable(glam::Mat4),
}

impl TransformCache {
    /// Determines transforms for all objects relative to a `reference_space`.
    /// I.e. the resulting transforms are "reference from scene"
    ///
    /// This means that the objects in `reference_space` get the identity transform and all other
    /// objects are transformed relative to it.
    ///
    /// Implementation note: We could do this also without `SpacesInfo`, but we assume that
    /// we already did the work to build up that datastructure, making the process here easier.
    #[allow(clippy::match_same_arms)]
    pub fn determine_transforms(
        spaces_info: &SpacesInfo,
        reference_space: &SpaceInfo,
        obj_properties: &ObjectsProperties,
    ) -> Self {
        crate::profile_function!();

        let mut transforms = Self {
            reference_from_obj_per_object: Default::default(),
        };

        // TODO(andreas): Should we be more selective about the objects we're actually interested in a given space view?
        //                  Ideally we'd be lazy, but we already have all these space infos around anyways...

        // Walk up from the reference space to the highest reachable parent.
        let mut topmost_reachable_space = reference_space;
        let mut reference_from_topmost = glam::Mat4::IDENTITY;
        while let Some((parent_path, parent_transform)) = topmost_reachable_space.parent.as_ref() {
            let Some(parent_space) = spaces_info.get(parent_path) else {
                break;
            };

            reference_from_topmost = match parent_transform {
                re_log_types::Transform::Rigid3(rigid) => {
                    reference_from_topmost * rigid.child_from_parent().to_mat4()
                }
                // If we're connected via 'unknown' it's not reachable
                re_log_types::Transform::Unknown => {
                    transforms.mark_self_and_parents_unreachable(
                        spaces_info,
                        parent_space,
                        UnreachableTransformReason::UnknownTransform,
                    );
                    break;
                }
                // We don't yet support reaching through pinhole.
                re_log_types::Transform::Pinhole(_) => {
                    transforms.mark_self_and_parents_unreachable(
                        spaces_info,
                        parent_space,
                        UnreachableTransformReason::ReferenceIsUnderPinhole,
                    );
                    break;
                }
            };
            topmost_reachable_space = parent_space;
        }

        // And then walk all branches down again.
        transforms.gather_child_transforms(
            spaces_info,
            topmost_reachable_space,
            obj_properties,
            reference_from_topmost,
            false,
        );
        transforms
    }

    fn register_transform_for(&mut self, space: &SpaceInfo, transform: &ReferenceFromObjTransform) {
        self.reference_from_obj_per_object.extend(
            space
                .descendants_without_transform
                .iter()
                .map(|obj| (*obj.hash(), transform.clone())),
        );
    }

    fn mark_self_and_parents_unreachable(
        &mut self,
        spaces_info: &SpacesInfo,
        space: &SpaceInfo,
        reason: UnreachableTransformReason,
    ) {
        self.register_transform_for(space, &ReferenceFromObjTransform::Unreachable(reason));

        if let Some((parent_path, _)) = &space.parent {
            if let Some(parent_space) = spaces_info.get(parent_path) {
                for sibling_path in parent_space.child_spaces.keys() {
                    if *sibling_path == space.path {
                        continue;
                    }
                    if let Some(child_space) = spaces_info.get(sibling_path) {
                        self.mark_self_and_children_unreachable(spaces_info, child_space, reason);
                    }
                }
                self.mark_self_and_parents_unreachable(spaces_info, parent_space, reason);
            }
        }
    }

    fn mark_self_and_children_unreachable(
        &mut self,
        spaces_info: &SpacesInfo,
        space: &SpaceInfo,
        reason: UnreachableTransformReason,
    ) {
        self.register_transform_for(space, &ReferenceFromObjTransform::Unreachable(reason));

        for child_path in space.child_spaces.keys() {
            if let Some(child_space) = spaces_info.get(child_path) {
                self.mark_self_and_children_unreachable(spaces_info, child_space, reason);
            }
        }
    }

    fn gather_child_transforms(
        &mut self,
        spaces_info: &SpacesInfo,
        space: &SpaceInfo,
        obj_properties: &ObjectsProperties,
        reference_from_obj: glam::Mat4,
        encountered_pinhole: bool,
    ) {
        self.register_transform_for(
            space,
            &ReferenceFromObjTransform::Reachable(reference_from_obj),
        );

        for (child_path, transform) in &space.child_spaces {
            if let Some(child_space) = spaces_info.get(child_path) {
                let mut encountered_pinhole = encountered_pinhole;
                let refererence_from_obj_in_child = match transform {
                    re_log_types::Transform::Rigid3(rigid) => {
                        reference_from_obj * rigid.parent_from_child().to_mat4()
                    }
                    // If we're connected via 'unknown' it's not reachable
                    re_log_types::Transform::Unknown => {
                        self.mark_self_and_children_unreachable(
                            spaces_info,
                            child_space,
                            UnreachableTransformReason::UnknownTransform,
                        );
                        continue;
                    }

                    re_log_types::Transform::Pinhole(pinhole) => {
                        if encountered_pinhole {
                            self.mark_self_and_children_unreachable(
                                spaces_info,
                                child_space,
                                UnreachableTransformReason::NestedPinholeCameras,
                            );
                            continue;
                        }
                        encountered_pinhole = true;

                        // A pinhole camera means that we're looking at an image.
                        // Images are spanned in their local x/y space with their r
                        // Center it and move it along z, scaling the further we move.

                        let distance = obj_properties
                            .get(child_path)
                            .pinhole_image_plane_distance(pinhole);

                        let scale = distance / pinhole.focal_length_in_pixels().y;
                        let translation = (-pinhole.principal_point() * scale).extend(distance);
                        let parent_from_child = glam::Mat4::from_scale_rotation_translation(
                            glam::vec3(scale, scale, 1.0),
                            glam::Quat::IDENTITY,
                            translation,
                        );

                        reference_from_obj * parent_from_child
                    }
                };

                self.gather_child_transforms(
                    spaces_info,
                    child_space,
                    obj_properties,
                    refererence_from_obj_in_child,
                    encountered_pinhole,
                );
            }
        }
    }

    /// Retrieves the transform of on object from its local system to the space of the reference.
    ///
    /// This is typically used as the "world space" for the renderer in a given frame.
    pub fn reference_from_obj(&self, obj_path: &ObjPath) -> ReferenceFromObjTransform {
        self.reference_from_obj_per_object
            .get(obj_path.hash())
            .cloned()
            .unwrap_or(ReferenceFromObjTransform::Unreachable(
                UnreachableTransformReason::Unconnected,
            ))
    }
}
