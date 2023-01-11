use glam::Mat4;
use re_data_store::{query_transform, InstanceIdHash, ObjPath};
use re_log_types::{IndexHash, Pinhole, Transform};
use re_query::{query_entity_with_primary, QueryError};

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{SceneSpatial, SpaceCamera3D},
    },
};

use super::ScenePart;

/// `ScenePart` for classic data path
pub struct CamerasPartClassic;

impl ScenePart for CamerasPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_scope!("CamerasPartClassic");

        // Atypical query. But gone soon anyways once everything is Arrow driven (where this isn't as special!)
        for (obj_path, props) in query.iter_entities() {
            // TODO(andreas): What about time ranges? See also https://github.com/rerun-io/rerun/issues/723
            let query_time = ctx.rec_cfg.time_ctrl.time_i64();
            let Some(Transform::Pinhole(pinhole)) = query_transform(
                    &ctx.log_db.obj_db,
                    &query.timeline,
                    obj_path,
                    query_time) else {
                continue;
            };
            let instance_hash = {
                if props.interactive {
                    InstanceIdHash::from_path_and_index(obj_path, IndexHash::NONE)
                } else {
                    InstanceIdHash::NONE
                }
            };

            CamerasPart::visit_instance(
                scene,
                obj_path,
                transforms,
                instance_hash,
                hovered_instance,
                pinhole,
            );
        }
    }
}

pub struct CamerasPart;

/*


/// Look for camera transform and pinhole in the transform hierarchy
/// and return them as cameras.
fn space_cameras(spaces_info: &SpacesInfo, space_info: &SpaceInfo) -> Vec<SpaceCamera3D> {
    crate::profile_function!();

    let mut space_cameras = vec![];

    for (child_path, child_transform) in &space_info.child_spaces {
        if let Transform::Rigid3(world_from_camera) = child_transform {
            let world_from_camera = world_from_camera.parent_from_child();

            let view_space = spaces_info
                .get(child_path)
                .and_then(|child| child.coordinates);

            if let Some(child_space_info) = spaces_info.get(child_path) {
                for (grand_child_path, grand_child_transform) in &child_space_info.child_spaces {
                    if let Transform::Pinhole(pinhole) = grand_child_transform {
                        space_cameras.push(SpaceCamera3D {
                            camera_obj_path: child_path.clone(),
                            instance_index_hash: re_log_types::IndexHash::NONE,
                            camera_view_coordinates: view_space,
                            world_from_camera,
                            pinhole: Some(*pinhole),
                            target_space: Some(grand_child_path.clone()),
                        });
                    }
                }
            }
        }
    }

    space_cameras
}



*/

impl CamerasPart {
    fn visit_instance(
        scene: &mut SceneSpatial,
        obj_path: &ObjPath,
        transforms: &TransformCache,
        instance: InstanceIdHash,
        hovered_instance: InstanceIdHash,
        pinhole: Pinhole,
    ) {
        // The transform *at* this object path already has the pinhole transformation we got passed in!
        // This makes sense, since if there's an image logged here one would expect that the transform applies.
        // We're however first interested in the rigid transform that led here, so query the parent transform.
        //
        // Note that currently a transform on an object can't have both a pinhole AND a rigid transform,
        // which makes this rather well defined here.
        let parent_path = obj_path
            .parent()
            .expect("root path can't be part of scene query");
        let ReferenceFromObjTransform::Reachable(world_from_parent) =
            transforms.reference_from_obj(&parent_path) else {
                return;
            };

        // If this transform is not representable as rigid transform, the camera is probably under another camera transform,
        // in which case we don't (yet) know how to deal with this!
        let Some(world_from_camera) = macaw::IsoTransform::from_mat4(&world_from_parent) else {
            return;
        };

        scene.space_cameras.push(SpaceCamera3D {
            camera_obj_path: obj_path.clone(),
            instance_index_hash: re_log_types::IndexHash::NONE,
            camera_view_coordinates: None, //  view_space, // TODO:
            world_from_camera,
            pinhole: Some(pinhole),
            target_space: Some(obj_path.clone()), // TODO: This is ALWAYS a pinhole camera. change space camera accordingly!
        });
    }
}

impl ScenePart for CamerasPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_scope!("CamerasPart");

        for (ent_path, props) in query.iter_entities() {
            let query = re_arrow_store::LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<Transform>(
                &ctx.log_db.obj_db.arrow_store,
                &query,
                ent_path,
                &[],
            )
            .and_then(|entity_view| {
                entity_view.visit(|instance, transform| {
                    let Transform::Pinhole(pinhole) = transform else {
                        return;
                    };

                    let instance_hash = {
                        if props.interactive {
                            InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                        } else {
                            InstanceIdHash::NONE
                        }
                    };

                    Self::visit_instance(
                        scene,
                        ent_path,
                        transforms,
                        instance_hash,
                        hovered_instance,
                        pinhole,
                    );
                })
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }
}
