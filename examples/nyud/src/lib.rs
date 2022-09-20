//! Example application based on <https://cs.nyu.edu/~silberman/datasets/nyu_depth_v2.html>.
//!
//! Uses the raw Rust communication primitives.

// TODO(emilk): translate this to use the Python SDK instead

#![allow(clippy::manual_range_contains)]

use std::path::Path;
use std::sync::mpsc::Sender;

use itertools::Itertools as _;
use re_log_types::*;

struct Logger<'a>(&'a Sender<LogMsg>);

impl<'a> Logger<'a> {
    fn log(&self, msg: impl Into<LogMsg>) {
        self.0.send(msg.into()).ok();
    }
}

pub fn log_dataset(path: &Path, tx: &Sender<LogMsg>) -> anyhow::Result<()> {
    let logger = Logger(tx);

    logger.log(BeginRecordingMsg {
        msg_id: MsgId::random(),
        info: RecordingInfo {
            recording_id: RecordingId::random(),
            started: Time::now(),
            recording_source: RecordingSource::Other("nyud".into()),
        },
    });

    logger.log(TypeMsg::obj_type(
        ObjTypePath::from("world"),
        ObjectType::Space,
    ));
    logger.log(TypeMsg::obj_type(
        ObjTypePath::from("depth"),
        ObjectType::Image,
    ));
    logger.log(TypeMsg::obj_type(
        ObjTypePath::from("rgb"),
        ObjectType::Image,
    ));
    logger.log(TypeMsg::obj_type(
        ObjTypePath::from("points"),
        ObjectType::Point3D,
    ));
    logger.log(TypeMsg::obj_type(
        ObjTypePath::from("camera"),
        ObjectType::Camera,
    ));

    configure_world_space(&logger);
    log_dataset_zip(path, &logger);

    Ok(())
}

fn configure_world_space(logger: &Logger<'_>) {
    logger.log(data_msg(
        &TimePoint::timeless(),
        ObjPath::from("world"),
        "up",
        Data::Vec3([0.0, -1.0, 0.0]),
    ));
}

fn log_dataset_zip(path: &Path, logger: &Logger<'_>) {
    let file = std::fs::File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let dir = select_first_dir(&mut archive);
    re_log::info!("Logging dir {dir:?}…");

    let mut file_contents = vec![];

    // logging depth images is slow, so we don't log every frame
    let mut depth_images_counter = 0;
    const DEPTH_IMAGE_INTERVAL: usize = 8;

    let points_obj_path = obj_path_vec!("points");

    {
        logger.log(data_msg(
            &TimePoint::timeless(),
            points_obj_path.clone(),
            "space",
            LoggedData::BatchSplat(Data::Space(ObjPath::from("world"))),
        ));

        // TODO(emilk): base color on depth?
        logger.log(data_msg(
            &TimePoint::timeless(),
            points_obj_path.clone(),
            "color",
            LoggedData::BatchSplat(Data::Color([255_u8; 4])),
        ));
    }

    for i in 0..archive.len() {
        let file = archive.by_index_raw(i).unwrap();
        let file_name = file.name().to_owned();
        if file.is_file()
            && file_name.starts_with(&dir)
            && (file_name.ends_with(".pgm") || file_name.ends_with(".ppm"))
        {
            re_log::debug!("{:?}…", file_name);
            drop(file);
            let mut file = archive.by_index(i).unwrap();

            file_contents.clear();
            std::io::copy(&mut file, &mut file_contents).unwrap();

            let file_name_parts = file_name.split('-').collect_vec();
            let time = file_name_parts[file_name_parts.len() - 2];
            let time = Time::from_seconds_since_epoch(time.parse().unwrap());
            let time_point = time_point([("time", TimeType::Time, time.into())]);

            if file_name.ends_with(".ppm") {
                let image = image::load_from_memory(&file_contents).unwrap().into_rgb8();

                let tensor = re_log_types::Tensor {
                    shape: vec![image.height() as _, image.width() as _, 3],
                    dtype: re_log_types::TensorDataType::U8,
                    data: TensorDataStore::Dense(image.to_vec().into()),
                };

                let obj_path = obj_path_vec!("rgb");
                logger.log(data_msg(&time_point, obj_path.clone(), "tensor", tensor));
                logger.log(data_msg(
                    &time_point,
                    obj_path,
                    "space",
                    Data::Space(ObjPath::from("image")),
                ));
            }

            let is_depth_image = file_name.ends_with(".pgm");
            depth_images_counter += is_depth_image as usize;
            if depth_images_counter % DEPTH_IMAGE_INTERVAL == 1 {
                let depth_image = image::load_from_memory(&file_contents)
                    .unwrap()
                    .into_luma16();

                let meter = 1e4; // TODO(emilk): unit of the depth data = what?

                {
                    let tensor = re_log_types::Tensor {
                        shape: vec![depth_image.height() as _, depth_image.width() as _],
                        dtype: re_log_types::TensorDataType::U16,
                        data: TensorDataStore::Dense(
                            bytemuck::cast_slice(depth_image.as_raw()).into(),
                        ),
                    };
                    let obj_path = obj_path_vec!("depth");
                    logger.log(data_msg(&time_point, obj_path.clone(), "tensor", tensor));
                    logger.log(data_msg(
                        &time_point,
                        obj_path.clone(),
                        "meter",
                        Data::F32(meter),
                    ));
                    logger.log(data_msg(
                        &time_point,
                        obj_path,
                        "space",
                        Data::Space(ObjPath::from("image")),
                    ));
                }

                let (w, h) = (depth_image.width(), depth_image.height());
                let f = 0.7 * w as f32; // whatever
                let intrinsics = glam::Mat3::from_cols_array_2d(&[
                    [f, 0.0, 0.0],                         // col 0
                    [0.0, f, 0.0],                         // col 1
                    [w as f32 / 2.0, h as f32 / 2.0, 1.0], // col 2
                ]);
                let world_from_pixel = intrinsics.inverse();

                let mut indices = Vec::with_capacity((w * h) as usize);
                let mut positions = Vec::with_capacity((w * h) as usize);

                for y in 0..h {
                    for x in 0..w {
                        let depth = depth_image.get_pixel(x, y)[0];
                        if depth < 3000 || 64000 < depth {
                            continue; // unreliable!
                        }

                        let depth = depth as f32 / meter;
                        let pos =
                            world_from_pixel * glam::Vec3::new(x as f32, y as f32, 1.0) * depth;

                        indices.push(Index::Pixel([x as _, y as _]));
                        positions.push(pos.to_array());
                    }
                }

                logger.log(data_msg(
                    &time_point,
                    points_obj_path.clone(),
                    "pos",
                    LoggedData::Batch {
                        indices: indices.clone(),
                        data: DataVec::Vec3(positions),
                    },
                ));

                {
                    let obj_path = obj_path_vec!("camera");
                    logger.log(data_msg(
                        &time_point,
                        obj_path.clone(),
                        "camera",
                        Data::Camera(Camera {
                            rotation: [0.0, 0.0, 0.0, 1.0],
                            position: [0.0, 0.0, 0.0],
                            camera_space_convention: CameraSpaceConvention::XRightYDownZFwd,
                            intrinsics: Some(intrinsics.to_cols_array_2d()),
                            resolution: Some([w as _, h as _]),
                            target_space: Some(ObjPath::from("image")),
                        }),
                    ));
                    logger.log(data_msg(
                        &time_point,
                        obj_path.clone(),
                        "space",
                        Data::Space(ObjPath::from("world")),
                    ));
                }
            }
        }
    }

    re_log::info!("Done logging {dir:?}.");
}

fn select_first_dir<R: std::io::Read + std::io::Seek>(archive: &mut zip::ZipArchive<R>) -> String {
    for i in 0..archive.len() {
        let file = archive.by_index_raw(i).unwrap();
        if file.is_dir() {
            return file.name().to_owned();
        }
    }
    panic!("No dir in the zip file");
}
