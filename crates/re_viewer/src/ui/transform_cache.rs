use nohash_hasher::IntMap;
use re_data_store::{ObjPath, ObjectsProperties};

use crate::misc::space_info::{SpaceInfo, SpacesInfo};

/// Provides transforms from an object to a chosen reference space for all elements in the scene.
///
/// The renderer then uses this reference space as its world space,
/// making world and reference space equivalent for a given space view.
#[derive(Clone, Default)]
pub struct TransformCache {
    reference_from_obj_per_object: IntMap<ObjPath, ReferenceFromObjTransform>,
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

#[derive(Clone)]
pub enum ReferenceFromObjTransform {
    /// On the path from the given object to the reference is an obstacle.
    Unreachable(UnreachableTransformReason),

    /// We're able to transform this object into the reference space.
    Reachable(glam::Mat4),
}

impl ReferenceFromObjTransform {
    pub fn is_reachable(&self) -> bool {
        match self {
            ReferenceFromObjTransform::Unreachable(_) => false,
            ReferenceFromObjTransform::Reachable(_) => true,
        }
    }
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
    pub fn determine_transforms(
        spaces_info: &SpacesInfo,
        reference_space: &SpaceInfo,
        obj_properties: &ObjectsProperties,
        scene_bbox: &macaw::BoundingBox,
    ) -> Self {
        crate::profile_function!();

        let mut transforms = Self {
            reference_from_obj_per_object: Default::default(),
        };

        // Child transforms of this space
        transforms.gather_descendents_transforms(
            spaces_info,
            reference_space,
            obj_properties,
            glam::Mat4::IDENTITY,
            false,
            None,
            scene_bbox,
        );

        // Walk up from the reference space to the highest reachable parent.
        let mut encountered_pinhole = false;
        let mut reference_from_ancestor = glam::Mat4::IDENTITY;
        let mut previous_space = reference_space;
        while let Some((parent_space, parent_transform)) = previous_space.parent(spaces_info) {
            reference_from_ancestor = match parent_transform {
                re_log_types::Transform::Rigid3(rigid) => {
                    reference_from_ancestor * rigid.child_from_parent().to_mat4()
                }
                // If we're connected via 'unknown' it's not reachable
                re_log_types::Transform::Unknown => {
                    parent_space.visit_non_descendants(spaces_info, &mut |space| {
                        transforms.register_transform_for(
                            space,
                            &ReferenceFromObjTransform::Unreachable(
                                UnreachableTransformReason::UnknownTransform,
                            ),
                        );
                    });
                    break;
                }

                re_log_types::Transform::Pinhole(pinhole) => {
                    if encountered_pinhole {
                        parent_space.visit_non_descendants(spaces_info, &mut |space| {
                            transforms.register_transform_for(
                                space,
                                &ReferenceFromObjTransform::Unreachable(
                                    UnreachableTransformReason::NestedPinholeCameras,
                                ),
                            );
                        });
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
                        parent_space.visit_non_descendants(spaces_info, &mut |space| {
                            transforms.register_transform_for(
                                space,
                                &ReferenceFromObjTransform::Unreachable(
                                    UnreachableTransformReason::InversePinholeCameraWithoutResolution,
                                ),
                            );
                        });
                        break;
                    }
                }
            };

            transforms.gather_descendents_transforms(
                spaces_info,
                parent_space,
                obj_properties,
                reference_from_ancestor,
                encountered_pinhole,
                Some(&previous_space.path),
                scene_bbox,
            );
            previous_space = parent_space;
        }

        transforms
    }

    fn register_transform_for(&mut self, space: &SpaceInfo, transform: &ReferenceFromObjTransform) {
        self.reference_from_obj_per_object.extend(
            space
                .descendants_without_transform
                .iter()
                .map(|obj| (obj.clone(), transform.clone())),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn gather_descendents_transforms(
        &mut self,
        spaces_info: &SpacesInfo,
        space: &SpaceInfo,
        obj_properties: &ObjectsProperties,
        reference_from_obj: glam::Mat4,
        encountered_pinhole: bool,
        skipped_child_path: Option<&ObjPath>,
        scene_bbox: &macaw::BoundingBox,
    ) {
        self.register_transform_for(
            space,
            &ReferenceFromObjTransform::Reachable(reference_from_obj),
        );

        for (child_path, transform) in &space.child_spaces {
            if skipped_child_path == Some(child_path) {
                continue;
            }

            if let Some(child_space) = spaces_info.get(child_path) {
                let mut encountered_pinhole = encountered_pinhole;
                let reference_from_obj_in_child = match transform {
                    re_log_types::Transform::Rigid3(rigid) => {
                        reference_from_obj * rigid.parent_from_child().to_mat4()
                    }
                    // If we're connected via 'unknown' it's not reachable
                    re_log_types::Transform::Unknown => {
                        child_space.visit_descendants(spaces_info, &mut |space| {
                            self.register_transform_for(
                                space,
                                &ReferenceFromObjTransform::Unreachable(
                                    UnreachableTransformReason::UnknownTransform,
                                ),
                            );
                        });
                        continue;
                    }

                    re_log_types::Transform::Pinhole(pinhole) => {
                        if encountered_pinhole {
                            child_space.visit_descendants(spaces_info, &mut |space| {
                                self.register_transform_for(
                                    space,
                                    &ReferenceFromObjTransform::Unreachable(
                                        UnreachableTransformReason::NestedPinholeCameras,
                                    ),
                                );
                            });
                            continue;
                        }
                        encountered_pinhole = true;

                        // A pinhole camera means that we're looking at an image.
                        // Images are spanned in their local x/y space.
                        // Center it and move it along z, scaling the further we move.

                        let distance = obj_properties
                            .get(child_path)
                            .pinhole_image_plane_distance(scene_bbox);

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
                };

                self.gather_descendents_transforms(
                    spaces_info,
                    child_space,
                    obj_properties,
                    reference_from_obj_in_child,
                    encountered_pinhole,
                    None,
                    scene_bbox,
                );
            }
        }
    }

    /// Retrieves the transform of on object from its local system to the space of the reference.
    ///
    /// This is typically used as the "world space" for the renderer in a given frame.
    pub fn reference_from_obj(&self, obj_path: &ObjPath) -> ReferenceFromObjTransform {
        self.reference_from_obj_per_object
            .get(obj_path)
            .cloned()
            .unwrap_or(ReferenceFromObjTransform::Unreachable(
                UnreachableTransformReason::Unconnected,
            ))
    }

    pub fn objects_with_reachable_transform(&self) -> impl Iterator<Item = &ObjPath> {
        self.reference_from_obj_per_object
            .iter()
            .filter_map(|(obj, transform)| match transform {
                ReferenceFromObjTransform::Unreachable(_) => None,
                ReferenceFromObjTransform::Reachable(_) => Some(obj),
            })
    }
}
