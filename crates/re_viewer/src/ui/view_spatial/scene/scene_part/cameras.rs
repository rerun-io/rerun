use re_data_store::{query_transform, InstanceIdHash, ObjPath};
use re_log_types::{
    coordinates::{Handedness, SignedAxis3},
    IndexHash, Pinhole, Transform, ViewCoordinates,
};
use re_query::{query_entity_with_primary, QueryError};

use crate::{
    misc::{space_info::query_view_coordinates, ViewerContext},
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

            let view_coordinates = determine_view_coordinates(
                &ctx.log_db.obj_db,
                &ctx.rec_cfg.time_ctrl,
                obj_path.clone(),
            );

            CamerasPart::visit_instance(
                scene,
                obj_path,
                transforms,
                instance_hash,
                hovered_instance,
                pinhole,
                view_coordinates,
            );
        }
    }
}

/// Determine the view coordinates (i.e.) the axis semantics.
///
/// The recommended way to log this is on the object holding the extrinsic camera properties
/// (i.e. the last rigid transform from here)
/// But for ease of use allow it everywhere along the path.
///
/// TODO(andreas): Doing a search upwards here isn't great. Maybe this can be part of the transform cache or similar?
fn determine_view_coordinates(
    obj_db: &re_data_store::log_db::ObjDb,
    time_ctrl: &crate::misc::TimeControl,
    mut obj_path: ObjPath,
) -> ViewCoordinates {
    loop {
        if let Some(view_coordinates) = query_view_coordinates(obj_db, time_ctrl, &obj_path) {
            return view_coordinates;
        }

        if let Some(parent) = obj_path.parent() {
            obj_path = parent;
        } else {
            // Keep in mind, there is no universal convention for any of this!
            // https://twitter.com/freyaholmer/status/1325556229410861056
            return ViewCoordinates::from_up_and_handedness(
                SignedAxis3::POSITIVE_Y,
                Handedness::Right,
            );
        }
    }
}

pub struct CamerasPart;

impl CamerasPart {
    fn visit_instance(
        scene: &mut SceneSpatial,
        obj_path: &ObjPath,
        transforms: &TransformCache,
        instance: InstanceIdHash,
        // TODO(andreas): Don't need hovered instances *yet* since most of the primitive handling is delayed.
        _hovered_instance: InstanceIdHash,
        pinhole: Pinhole,
        view_coordinates: ViewCoordinates,
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

        // Actual primitives are generated later.
        // Currently, we need information about viewport to display it correctly.
        // TODO(andreas): Would be great if we add all the lines etc. right away!
        //                  Let's attempt this as part of
        //                  https://github.com/rerun-io/rerun/issues/681 (Improve camera frustum length heuristic & editability)
        //                  and https://github.com/rerun-io/rerun/issues/686 (Replace camera mesh with expressive camera gizmo (extension of current frustum)
        scene.space_cameras.push(SpaceCamera3D {
            obj_path: obj_path.clone(),
            instance,
            view_coordinates,
            world_from_camera,
            pinhole: Some(pinhole),
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
                entity_view.visit1(|instance, transform| {
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

                    let view_coordinates = determine_view_coordinates(
                        &ctx.log_db.obj_db,
                        &ctx.rec_cfg.time_ctrl,
                        ent_path.clone(),
                    );

                    Self::visit_instance(
                        scene,
                        ent_path,
                        transforms,
                        instance_hash,
                        hovered_instance,
                        pinhole,
                        view_coordinates,
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
