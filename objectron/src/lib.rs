pub mod protos;

use std::sync::mpsc::Sender;
use std::{
    io::Read,
    path::{Path, PathBuf},
};

use image::ImageDecoder;
use itertools::Itertools as _;
use log_types::*;
use protos::{ArCamera, ArPlaneAnchor};

struct Logger<'a>(&'a Sender<LogMsg>);

impl<'a> Logger<'a> {
    fn log(&self, msg: impl Into<LogMsg>) {
        self.0.send(msg.into()).ok();
    }
}

pub fn log_dataset(path: &Path, tx: &Sender<LogMsg>) -> anyhow::Result<()> {
    let logger = Logger(tx);

    logger.log(TypeMsg::object_type(
        ObjTypePath::from("world"),
        ObjectType::Space,
    ));
    logger.log(TypeMsg::object_type(
        ObjTypePath::from("camera"),
        ObjectType::Camera,
    ));
    logger.log(TypeMsg::object_type(
        ObjTypePath::from("video"),
        ObjectType::Image,
    ));
    logger.log(TypeMsg::object_type(
        ObjTypePath::from("rgb"),
        ObjectType::Image,
    ));
    logger.log(TypeMsg::object_type(
        ObjTypePath::from("points") / TypePathComp::Index,
        ObjectType::Point3D,
    ));
    logger.log(TypeMsg::object_type(
        ObjTypePath::from("objects") / TypePathComp::Index / "bbox2d",
        ObjectType::LineSegments2D,
    ));
    logger.log(TypeMsg::object_type(
        ObjTypePath::from("objects") / TypePathComp::Index / "bbox3d",
        ObjectType::Box3D,
    ));

    let frame_times = log_geometry_pbdata(path, &logger)?;
    configure_world_space(&logger);
    log_annotation_pbdata(path, &frame_times, &logger)?;

    Ok(())
}

fn configure_world_space(logger: &Logger<'_>) {
    let world_space = ObjPath::from(ObjPathBuilder::from("world"));
    // TODO: what time point should we use?
    let time_point = time_point([
        ("frame", TimeValue::Sequence(0)),
        ("time", TimeValue::Time(Time::from_seconds_since_epoch(0.0))),
    ]);
    logger.log(data_msg(
        &time_point,
        world_space,
        "up",
        Data::Vec3([0.0, 1.0, 0.0]),
    ));
}

fn log_annotation_pbdata(
    path: &Path,
    frame_times: &[Time],
    logger: &Logger<'_>,
) -> anyhow::Result<()> {
    use prost::Message as _;

    let file = std::fs::read(path.join("annotation.pbdata"))?;
    let sequence = protos::Sequence::decode(file.as_slice())?;
    let world_space = ObjPath::from(ObjPathBuilder::from("world"));
    let image_space = ObjPath::from(ObjPathBuilder::from("image"));

    for object in &sequence.objects {
        // TODO: what time point should we use?
        let time_point = time_point([
            ("frame", TimeValue::Sequence(0)),
            ("time", TimeValue::Time(Time::from_seconds_since_epoch(0.0))),
        ]);

        let data_path = ObjPathBuilder::from("objects") / Index::Integer(object.id as _);

        // dbg!(&object.category); // Most cups have "chair" as the category

        if object.r#type == protos::object::Type::BoundingBox as i32 {
            let rotation = glam::Mat3::from_cols_slice(&object.rotation).transpose();
            let rotation = glam::Quat::from_mat3(&rotation);
            let translation = glam::Vec3::from_slice(&object.translation);
            let half_size = glam::Vec3::from_slice(&object.scale);
            let box3 = Box3 {
                rotation: rotation.to_array(),
                translation: translation.to_array(),
                half_size: half_size.to_array(),
            };
            let obj_path = &data_path / "bbox3d";
            logger.log(data_msg(&time_point, &obj_path, "obb", box3));
            logger.log(data_msg(
                &time_point,
                obj_path,
                "space",
                world_space.clone(),
            ));
        } else {
            tracing::error!("Unsupported type: {}", object.r#type);
        }

        logger.log(data_msg(
            &time_point,
            &data_path / "bbox3d",
            "color",
            Data::Color([130, 160, 250, 255]),
        ));
    }

    for frame_annotation in sequence.frame_annotations.iter() {
        let frame_idx = frame_annotation.frame_id as _;
        // let time = Time::from_seconds_since_epoch(frame_annotation.timestamp); // this is always zero :(
        let time = frame_times[frame_idx as usize];
        let time_point = time_point([
            ("frame", TimeValue::Sequence(frame_idx)),
            ("time", TimeValue::Time(time)),
        ]);

        for object_annotation in &frame_annotation.annotations {
            let data_path =
                ObjPathBuilder::from("objects") / Index::Integer(object_annotation.object_id as _);

            // always zero?
            // logger.log(data_msg(
            //     &time_point,
            //     &data_path / "visibility",
            //     object_annotation.visibility,
            // ))
            // ;

            let mut keypoint_ids = vec![];
            let mut keypoints_2d = vec![];

            for keypoint in &object_annotation.keypoints {
                if let Some(point_2d) = &keypoint.point_2d {
                    let pos2 = [point_2d.x * 1440.0, point_2d.y * 1920.0]; // TODO: remove hack
                    keypoint_ids.push(keypoint.id);
                    keypoints_2d.push(pos2);
                    // TODO: log depth too
                }
            }

            if keypoints_2d.len() == 9 {
                // Bounding box. First point is center.
                let line_segments = vec![
                    [keypoints_2d[1], keypoints_2d[2]],
                    [keypoints_2d[1], keypoints_2d[3]],
                    [keypoints_2d[4], keypoints_2d[2]],
                    [keypoints_2d[4], keypoints_2d[3]],
                    //
                    [keypoints_2d[5], keypoints_2d[6]],
                    [keypoints_2d[5], keypoints_2d[7]],
                    [keypoints_2d[8], keypoints_2d[6]],
                    [keypoints_2d[8], keypoints_2d[7]],
                    //
                    [keypoints_2d[1], keypoints_2d[5]],
                    [keypoints_2d[2], keypoints_2d[6]],
                    [keypoints_2d[3], keypoints_2d[7]],
                    [keypoints_2d[4], keypoints_2d[8]],
                ];

                let obj_path = &data_path / "bbox2d";
                logger.log(data_msg(
                    &time_point,
                    &obj_path,
                    "line_segments",
                    Data::LineSegments2D(line_segments),
                ));
                logger.log(data_msg(
                    &time_point,
                    &obj_path,
                    "space",
                    image_space.clone(),
                ));

                logger.log(data_msg(
                    &time_point,
                    obj_path,
                    "color",
                    Data::Color([130, 160, 250, 255]),
                ));
            } else {
                for (id, pos2) in keypoint_ids.into_iter().zip(keypoints_2d) {
                    let point_path = &data_path / "points" / Index::Integer(id as _);
                    logger.log(data_msg(
                        &time_point,
                        &point_path,
                        "pos2d",
                        Data::Vec2(pos2),
                    ));
                    logger.log(data_msg(
                        &time_point,
                        point_path,
                        "space",
                        image_space.clone(),
                    ));
                }
            }
        }
    }

    Ok(())
}

fn log_geometry_pbdata(path: &Path, logger: &Logger<'_>) -> anyhow::Result<Vec<Time>> {
    let file = std::fs::File::open(path.join("geometry.pbdata"))?;
    let mut reader = std::io::BufReader::with_capacity(1024, file);

    let mut msg_len = [0_u8; 4];
    let mut msg = vec![];

    let mut frame_idx = 0;

    let world_space = ObjPath::from(ObjPathBuilder::from("world"));

    let mut frame_times = vec![];

    while reader.read_exact(&mut msg_len).is_ok() {
        let msg_len = u32::from_le_bytes(msg_len);
        msg.resize(msg_len as usize, 0);
        reader.read_exact(&mut msg)?;

        use prost::Message as _;
        let ar_frame = protos::ArFrame::decode(msg.as_slice())?;

        let time = Time::from_seconds_since_epoch(ar_frame.timestamp.unwrap());
        frame_times.push(time);
        let time_point = time_point([
            ("frame", TimeValue::Sequence(frame_idx)),
            ("time", TimeValue::Time(time)),
        ]);

        log_image(
            &path.join("video").join(format!("{frame_idx}.jpg")),
            &time_point,
            logger,
        )?;

        if let Some(ar_camera) = &ar_frame.camera {
            log_ar_camera(
                &time_point,
                ObjPathBuilder::from("camera"),
                &world_space,
                ar_camera,
                logger,
            );
        }

        if false {
            // The planes are almost always really bad, and sometimes very far away.
            for plane_anchor in &ar_frame.plane_anchor {
                // TODO: we shouldn't need to explicitly group planes and points like this! (we do it so we can toggle their visibility all at once).
                log_plane_anchor(
                    &time_point,
                    &ObjPathBuilder::from("planes"),
                    &world_space,
                    plane_anchor,
                    logger,
                );
            }
        }

        if let Some(raw_feature_points) = &ar_frame.raw_feature_points {
            if let Some(count) = raw_feature_points.count {
                // TODO: we shouldn't need to explicitly group planes and points like this! (we do it so we can toggle their visibility all at once).
                let points_path = ObjPathBuilder::from("points");

                for i in 0..count as usize {
                    let point = &raw_feature_points.point[i];
                    let identifier = raw_feature_points.identifier[i];

                    if let (Some(x), Some(y), Some(z)) = (point.x, point.y, point.z) {
                        let point_path = &points_path / Index::Integer(identifier as _);

                        logger.log(data_msg(
                            &time_point,
                            &point_path,
                            "pos",
                            Data::Vec3([x, y, z]),
                        ));
                        logger.log(data_msg(
                            &time_point,
                            &point_path,
                            "space",
                            world_space.clone(),
                        ));
                        // TODO: log once for the parent ("points")
                        logger.log(data_msg(
                            &time_point,
                            &point_path,
                            "color",
                            Data::Color([255; 4]),
                        ));
                    }
                }

                // TODO: project the points onto 2D image plane and log,
                // or log a transform between world and image space
            }
        }

        frame_idx += 1;
    }

    Ok(frame_times)
}

fn log_image(path: &PathBuf, time_point: &TimePoint, logger: &Logger<'_>) -> anyhow::Result<()> {
    let image_space = ObjPath::from(ObjPathBuilder::from("image"));

    let data = std::fs::read(path)?;

    let (w, h) = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(&data))?.dimensions();

    let image = log_types::Image {
        format: log_types::ImageFormat::Jpeg,
        size: [w, h],
        data,
    };

    let obj_path = ObjPathBuilder::from("video");
    logger.log(data_msg(time_point, obj_path.clone(), "image", image));
    logger.log(data_msg(time_point, obj_path, "space", image_space));

    Ok(())
}

fn log_ar_camera(
    time_point: &TimePoint,
    data_path: ObjPathBuilder,
    world_space: &ObjPath,
    ar_camera: &ArCamera,
    logger: &Logger<'_>,
) {
    let world_from_cam = glam::Mat4::from_cols_slice(&ar_camera.transform).transpose();
    let (scale, rot, translation) = world_from_cam.to_scale_rotation_translation();
    assert!((scale - glam::Vec3::ONE).length() < 1e-3);
    let intrinsics = glam::Mat3::from_cols_slice(&ar_camera.intrinsics).transpose();
    let w = ar_camera.image_resolution_width.unwrap() as f32;
    let h = ar_camera.image_resolution_height.unwrap() as f32;

    // Because the dataset is collected in portrait:
    let swizzle_x_y = glam::Mat3::from_cols_array_2d(&[[0., 1., 0.], [1., 0., 0.], [0., 0., 1.]]);
    let intrinsics = swizzle_x_y * intrinsics * swizzle_x_y;
    let rot = rot * glam::Quat::from_axis_angle(glam::Vec3::Z, std::f32::consts::TAU / 4.0);
    let [w, h] = [h, w];

    let camera = log_types::Camera {
        rotation: rot.into(),
        position: translation.into(),
        intrinsics: Some(intrinsics.to_cols_array_2d()),
        resolution: Some([w, h]),
    };

    logger.log(data_msg(time_point, &data_path, "camera", camera));
    logger.log(data_msg(
        time_point,
        data_path,
        "space",
        world_space.clone(),
    ));
}

fn log_plane_anchor(
    time_point: &TimePoint,
    root_path: &ObjPathBuilder,
    world_space: &ObjPath,
    plane_anchor: &ArPlaneAnchor,
    logger: &Logger<'_>,
) {
    let transform = glam::Mat4::from_cols_slice(&plane_anchor.transform).transpose();

    let identifier = plane_anchor.identifier.clone().unwrap();
    let plane_path = root_path / "planes" / Index::from(identifier);

    if let Some(plane_geometry) = &plane_anchor.geometry {
        let positions = plane_geometry
            .vertices
            .iter()
            .map(|p| {
                let p = [p.x.unwrap(), p.y.unwrap(), p.z.unwrap()];
                if true {
                    transform.project_point3(p.into()).into()
                } else {
                    p
                }
            })
            .collect();
        let indices = plane_geometry
            .triangle_indices
            .iter()
            .copied()
            .tuple_windows()
            .map(|(a, b, c)| [a as u32, b as u32, c as u32])
            .collect();
        let mesh = RawMesh3D { positions, indices };
        logger.log(data_msg(time_point, &plane_path, "mesh", Mesh3D::Raw(mesh)));
        logger.log(data_msg(
            time_point,
            &plane_path,
            "space",
            world_space.clone(),
        ));
    }
}
