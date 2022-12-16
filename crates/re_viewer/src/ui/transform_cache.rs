use nohash_hasher::IntMap;
use re_data_store::ObjPath;
use re_log_types::ObjPathHash;

use crate::misc::space_info::{SpaceInfo, SpacesInfo};

/// Provides transforms from scene to world space for all elements in the scene.
#[derive(Default)]
pub struct TransformCache {
    transforms: IntMap<ObjPathHash, glam::Mat4>,
}

impl TransformCache {
    /// Determines transforms for all objects relative to a `reference_space`.
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
            space_from_parent: glam::Mat4,
            transforms: &mut IntMap<ObjPathHash, glam::Mat4>,
        ) {
            // We default to identity transform, so no need to store objects on the reference space
            if space_from_parent != glam::Mat4::IDENTITY {
                transforms.extend(
                    space
                        .descendants_without_transform
                        .iter()
                        .map(|obj| (*obj.hash(), space_from_parent)),
                );
            }

            for (child_path, transform) in &space.child_spaces {
                if let Some(child_space) = spaces_info.spaces.get(child_path) {
                    let child_from_parent = match transform {
                        // Assume identity until told otherwise!
                        re_log_types::Transform::Unknown => glam::Mat4::IDENTITY,

                        re_log_types::Transform::Rigid3(rigid) => {
                            rigid.child_from_parent().to_mat4()
                        }
                        re_log_types::Transform::Pinhole(pinhole) => glam::Mat4::from_mat3(
                            glam::Mat3::from_cols_array_2d(&pinhole.image_from_cam),
                        ),
                    };

                    gather_child_transforms(
                        spaces_info,
                        child_space,
                        child_from_parent * space_from_parent,
                        transforms,
                    );
                }
            }
        }

        let mut transforms = Default::default();
        gather_child_transforms(
            spaces_info,
            reference_space,
            glam::Mat4::IDENTITY,
            &mut transforms,
        );
        // TODO: Walk up the parent

        Self { transforms }
    }

    pub fn get_world_from_scene(&self, obj_path: &ObjPath) -> &glam::Mat4 {
        self.transforms
            .get(obj_path.hash())
            .unwrap_or(&glam::Mat4::IDENTITY)
    }
}
