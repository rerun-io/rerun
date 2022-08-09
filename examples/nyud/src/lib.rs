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

    logger.log(TypeMsg::object_type(
        ObjTypePath::from("world"),
        ObjectType::Space,
    ));
    logger.log(TypeMsg::object_type(
        ObjTypePath::from("depth"),
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

    configure_world_space(&logger);
    log_dataset_zip(path, &logger);

    Ok(())
}

fn configure_world_space(logger: &Logger<'_>) {
    // TODO(emilk): what time point should we use?
    let time_point = time_point([("time", TimeValue::Time(Time::from_seconds_since_epoch(0.0)))]);
    logger.log(data_msg(
        &time_point,
        ObjPath::from("world"),
        "up",
        Data::Vec3([0.0, -1.0, 0.0]),
    ));
}

fn log_dataset_zip(path: &Path, logger: &Logger<'_>) {
    let file = std::fs::File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let dir = select_first_dir(&mut archive);
    tracing::info!("Logging dir {:?}", dir);

    let mut file_contents = vec![];

    // logging depth images is slow, so we don't log every frame
    let mut depth_images_counter = 0;
    const DEPTH_IMAGE_INTERVAL: usize = 8;

    let obj_path = ObjPathBuilder::from("points") / Index::Placeholder;

    {
        // TODO(emilk): better way to do "forever and always"
        let time_point =
            time_point([("time", TimeValue::Time(Time::from_seconds_since_epoch(0.0)))]);

        logger.log(data_msg(
            &time_point,
            &obj_path,
            "space",
            LoggedData::BatchSplat(Data::Space(ObjPath::from("world"))),
        ));

        // TODO(emilk): base color on depth?
        logger.log(data_msg(
            &time_point,
            &obj_path,
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
            tracing::debug!("{:?}…", file_name);
            drop(file);
            let mut file = archive.by_index(i).unwrap();

            file_contents.clear();
            std::io::copy(&mut file, &mut file_contents).unwrap();

            let file_name_parts = file_name.split('-').collect_vec();
            let time = file_name_parts[file_name_parts.len() - 2];
            let time = Time::from_seconds_since_epoch(time.parse().unwrap());
            let time_point = time_point([("time", TimeValue::Time(time))]);

            if file_name.ends_with(".ppm") {
                let image = image::load_from_memory(&file_contents).unwrap().into_rgb8();

                let tensor = re_log_types::Tensor {
                    shape: vec![image.height() as _, image.width() as _, 3],
                    dtype: re_log_types::TensorDataType::U8,
                    data: TensorDataStore::Dense(image.to_vec()),
                };

                let obj_path = ObjPathBuilder::from("rgb");
                logger.log(data_msg(&time_point, &obj_path, "tensor", tensor));
                logger.log(data_msg(
                    &time_point,
                    obj_path,
                    "space",
                    ObjPath::from("image"),
                ));
            }

            let is_depth_image = file_name.ends_with(".pgm");
            depth_images_counter += is_depth_image as usize;
            if depth_images_counter % DEPTH_IMAGE_INTERVAL == 1 {
                let depth_image = image::load_from_memory(&file_contents)
                    .unwrap()
                    .into_luma16();

                {
                    let tensor = re_log_types::Tensor {
                        shape: vec![depth_image.height() as _, depth_image.width() as _],
                        dtype: re_log_types::TensorDataType::U16,
                        data: TensorDataStore::Dense(
                            bytemuck::cast_slice(depth_image.as_raw()).to_vec(),
                        ),
                    };
                    let obj_path = ObjPathBuilder::from("depth");
                    logger.log(data_msg(&time_point, &obj_path, "tensor", tensor));
                    logger.log(data_msg(
                        &time_point,
                        obj_path,
                        "space",
                        ObjPath::from("depth_image"),
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

                        let depth = depth as f32 * 3e-5; // TODO(emilk): unit of the depth data = what?
                        let pos = world_from_pixel * glam::Vec3::new(x as f32, y as f32, depth);

                        indices.push(Index::Pixel([x as _, y as _]));
                        positions.push(pos.to_array());
                    }
                }

                logger.log(data_msg(
                    &time_point,
                    &obj_path,
                    "pos",
                    LoggedData::Batch {
                        indices: indices.clone(),
                        data: DataVec::Vec3(positions),
                    },
                ));
            }
        }
    }

    tracing::info!("Done!");
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
