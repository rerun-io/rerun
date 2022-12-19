use nohash_hasher::IntMap;
use re_data_store::ObjPath;
use re_log_types::ObjPathHash;

use crate::misc::space_info::{SpaceInfo, SpacesInfo};

/// Provides transforms from local to a reference space for all elements in the scene.
#[derive(Default)]
pub struct TransformCache {
    reference_from_local_per_object: IntMap<ObjPathHash, glam::Mat4>,
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
    ) -> TransformCache {
        // TODO(andreas): Should we be more selective about the objects we're actually interested in a given space view?
        //                  Ideally we'd be lazy, but we already have all these space infos around anyways...

        fn gather_child_transforms(
            spaces_info: &SpacesInfo,
            space: &SpaceInfo,
            reference_from_local: glam::Mat4,
            reference_from_local_per_object: &mut IntMap<ObjPathHash, glam::Mat4>,
        ) {
            reference_from_local_per_object.extend(
                space
                    .descendants_without_transform
                    .iter()
                    .map(|obj| (*obj.hash(), reference_from_local)),
            );

            for (child_path, transform) in &space.child_spaces {
                if let Some(child_space) = spaces_info.spaces.get(child_path) {
                    let child_from_parent = match transform {
                        // Assume identity until told otherwise!
                        re_log_types::Transform::Unknown => glam::Mat4::IDENTITY,

                        re_log_types::Transform::Rigid3(rigid) => {
                            rigid.child_from_parent().to_mat4()
                        }

                        // TODO(andreas): We don't support adding objects with pinhole yet.
                        // Need to think about this a bit more and test it.
                        re_log_types::Transform::Pinhole(pinhole) => glam::Mat4::from_mat3(
                            glam::Mat3::from_cols_array_2d(&pinhole.image_from_cam),
                        ),
                    };

                    gather_child_transforms(
                        spaces_info,
                        child_space,
                        child_from_parent * reference_from_local,
                        reference_from_local_per_object,
                    );
                }
            }
        }

        // Walk up to the highest reachable parent.
        let mut top_most_reachable_space = reference_space;
        let mut reference_from_local = glam::Mat4::IDENTITY;
        while let Some((parent_path, parent_transform)) = top_most_reachable_space.parent.as_ref() {
            let Some(parent_space) = spaces_info.spaces.get(parent_path) else {
                break;
            };

            reference_from_local = match parent_transform {
                // Assume identity until told otherwise!
                re_log_types::Transform::Unknown => reference_from_local,

                re_log_types::Transform::Rigid3(rigid) => {
                    reference_from_local * rigid.parent_from_child().to_mat4()
                }

                // TODO(andreas): Not supported yet.
                re_log_types::Transform::Pinhole(_) => {
                    break;
                }
            };
            top_most_reachable_space = parent_space;
        }

        // And then walk all branches down again.
        let mut reference_from_local_per_object = Default::default();
        gather_child_transforms(
            spaces_info,
            top_most_reachable_space,
            reference_from_local,
            &mut reference_from_local_per_object,
        );

        Self {
            reference_from_local_per_object,
        }
    }

    /// Retrieves the transform of on object from its local system to the space of the reference.
    ///
    /// This is typically used as the "world space" for the renderer in a given frame.
    pub fn reference_from_local(&self, obj_path: &ObjPath) -> &glam::Mat4 {
        self.reference_from_local_per_object
            .get(obj_path.hash())
            .unwrap_or(&glam::Mat4::IDENTITY)
    }
}
