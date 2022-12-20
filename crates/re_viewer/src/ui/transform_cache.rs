use nohash_hasher::IntMap;
use re_data_store::ObjPath;
use re_log_types::ObjPathHash;

use crate::misc::space_info::{SpaceInfo, SpacesInfo};

/// Provides transforms from an object to a chosen reference space for all elements in the scene.
///
/// The renderer then uses this reference space as its world space,
/// making world and reference space equivalent for a given space view.
pub struct TransformCache {
    reference_from_obj_per_object: IntMap<ObjPathHash, ReferenceFromObjTransform>,
}

#[derive(Clone)]
pub enum ReferenceFromObjTransform {
    /// On the path from the given object to the reference is an unknown transformation or a pinhole transformation
    ///
    /// TODO(andreas): Will need to be split up and we should be able to handle some of these cases!
    ConnectedViaUnknownOrPinhole,

    /// There is a rigid connection to the reference.
    Rigid(glam::Mat4),
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
    ) -> TransformCache {
        // TODO(andreas): Should we be more selective about the objects we're actually interested in a given space view?
        //                  Ideally we'd be lazy, but we already have all these space infos around anyways...

        fn gather_child_transforms(
            spaces_info: &SpacesInfo,
            space: &SpaceInfo,
            reference_from_obj: glam::Mat4,
            reference_from_obj_per_path: &mut IntMap<ObjPathHash, ReferenceFromObjTransform>,
        ) {
            reference_from_obj_per_path.extend(space.descendants_without_transform.iter().map(
                |obj| {
                    (
                        *obj.hash(),
                        ReferenceFromObjTransform::Rigid(reference_from_obj),
                    )
                },
            ));

            for (child_path, transform) in &space.child_spaces {
                if let Some(child_space) = spaces_info.spaces.get(child_path) {
                    let refererence_from_obj_in_child = match transform {
                        re_log_types::Transform::Rigid3(rigid) => {
                            reference_from_obj * rigid.parent_from_child().to_mat4()
                        }
                        // If we're connected via 'unknown' it's not reachable
                        re_log_types::Transform::Unknown => {
                            continue;
                        }
                        // We don't yet support reaching through pinhole.
                        re_log_types::Transform::Pinhole(_) => {
                            continue;
                        }
                    };

                    gather_child_transforms(
                        spaces_info,
                        child_space,
                        refererence_from_obj_in_child,
                        reference_from_obj_per_path,
                    );
                }
            }
        }

        // Walk up from the reference space to the highest reachable parent.
        let mut topmost_reachable_space = reference_space;
        let mut reference_from_topmost = glam::Mat4::IDENTITY;
        while let Some((parent_path, parent_transform)) = topmost_reachable_space.parent.as_ref() {
            let Some(parent_space) = spaces_info.spaces.get(parent_path) else {
                break;
            };

            reference_from_topmost = match parent_transform {
                re_log_types::Transform::Rigid3(rigid) => {
                    reference_from_topmost * rigid.child_from_parent().to_mat4()
                }
                // If we're connected via 'unknown' it's not reachable
                re_log_types::Transform::Unknown => {
                    break;
                }
                // We don't yet support reaching through pinhole.
                re_log_types::Transform::Pinhole(_) => {
                    break;
                }
            };
            topmost_reachable_space = parent_space;
        }

        // And then walk all branches down again.
        let mut reference_from_obj_per_object = Default::default();
        gather_child_transforms(
            spaces_info,
            topmost_reachable_space,
            reference_from_topmost,
            &mut reference_from_obj_per_object,
        );

        Self {
            reference_from_obj_per_object,
        }
    }

    /// Retrieves the transform of on object from its local system to the space of the reference.
    ///
    /// This is typically used as the "world space" for the renderer in a given frame.
    pub fn reference_from_obj(&self, obj_path: &ObjPath) -> ReferenceFromObjTransform {
        self.reference_from_obj_per_object
            .get(obj_path.hash())
            .cloned()
            .unwrap_or(ReferenceFromObjTransform::ConnectedViaUnknownOrPinhole)
    }
}
