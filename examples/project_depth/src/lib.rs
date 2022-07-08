#![allow(clippy::manual_range_contains)]

use std::sync::mpsc::Sender;

use re_viewer::math::{remap, remap_clamp};

use glam::*;
use itertools::Itertools as _;
use macaw::*;
use re_log_types::*;

struct Logger<'a>(&'a Sender<LogMsg>);

impl<'a> Logger<'a> {
    fn log(&self, msg: impl Into<LogMsg>) {
        self.0.send(msg.into()).ok();
    }
}

fn configure_world_space(logger: &Logger<'_>) {
    let world_space = ObjPathBuilder::from("world");
    // TODO: what time point should we use?
    let time_point = time_point([("time", TimeValue::Time(Time::from_seconds_since_epoch(0.0)))]);
    logger.log(data_msg(
        &time_point,
        &world_space,
        "up",
        Data::Vec3([0.0, 0.0, 1.0]),
    ));
}

struct Point {
    pos: glam::Vec3,
    color: [u8; 4],
}

pub fn log(tx: &Sender<LogMsg>) {
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
        ObjTypePath::from("camera"),
        ObjectType::Camera,
    ));
    logger.log(TypeMsg::object_type(
        ObjTypePath::from("points") / TypePathComp::Index,
        ObjectType::Point3D,
    ));

    configure_world_space(&logger);

    let time_point = time_point([("time", TimeValue::Time(Time::from_seconds_since_epoch(0.0)))]);

    let color_map = image::load_from_memory(include_bytes!("viridis.png"))
        .unwrap()
        .to_rgb8();

    {
        let depth_image =
            image::load_from_memory(include_bytes!("../plants_depth_mm.png")).unwrap();
        let mut depth_image = depth_image.into_luma16();

        // brighten it:
        for pixel in depth_image.pixels_mut() {
            pixel.0[0] *= 100;
        }

        let image = re_log_types::Image {
            size: [depth_image.width(), depth_image.height()],
            format: re_log_types::ImageFormat::Luminance16,
            data: bytemuck::cast_slice(depth_image.as_raw()).to_vec(),
        };
        let obj_path = ObjPathBuilder::from("depth");
        logger.log(data_msg(&time_point, &obj_path, "image", image));
        logger.log(data_msg(
            &time_point,
            obj_path,
            "space",
            ObjPath::from("depth_image"),
        ));
    }

    let depth_image = image::load_from_memory(include_bytes!("../plants_depth_mm.png")).unwrap();
    let depth_image = depth_image.flipv(); // TODO: why do we need this
    let depth_image = depth_image.into_luma16();

    let rgba_image = image::load_from_memory(include_bytes!("../plants_rgb.png")).unwrap();
    let rgba_image = rgba_image.flipv(); // TODO: why do we need this
    let rgba_image = rgba_image.into_rgba8();

    let [w, h] = [depth_image.width(), depth_image.height()];

    assert_eq!(rgba_image.width(), depth_image.width());
    assert_eq!(rgba_image.height(), depth_image.height());

    // dbg!(w, h);

    let closest_leaf_depth = 0.4; // Estimated by eye

    let up = Vec3::Z;
    let eye = vec3(0.0, -0.2, 0.7);
    let target = vec3(0.0, 0.0, 0.0);
    let view_from_world = IsoTransform::look_at_rh(eye, target, up).unwrap();
    let camera_plane = macaw::Plane3::from_normal_point((target - eye).normalize(), eye);
    let world_from_view = view_from_world.inverse();
    let fov_y_radians = 40.0_f32.to_radians();
    let aspect_ratio = 1.0;
    let clip_from_view = Mat4::perspective_infinite_rh(fov_y_radians, aspect_ratio, 0.01);
    let clip_from_world = clip_from_view * view_from_world.to_mat4();
    let world_from_clip = clip_from_world.inverse();

    {
        let f = 0.9 * w as f32; // TODO
        let intrinsics = glam::Mat3::from_cols_array_2d(&[
            [f, 0.0, 0.0],                         // col 0
            [0.0, f, 0.0],                         // col 1
            [w as f32 / 2.0, h as f32 / 2.0, 1.0], // col 2
        ]);

        let camera = re_log_types::Camera {
            rotation: world_from_view.rotation().into(),
            position: world_from_view.translation().into(),
            intrinsics: Some(intrinsics.to_cols_array_2d()),
            resolution: Some([w as f32, h as f32]),
        };
        let obj_path = ObjPathBuilder::from("camera");
        logger.log(data_msg(
            &time_point,
            &obj_path,
            "camera",
            Data::Camera(camera),
        ));
        logger.log(data_msg(
            &time_point,
            &obj_path,
            "space",
            Data::Space(ObjPath::from("world")),
        ));
        logger.log(data_msg(
            &time_point,
            &obj_path,
            "color",
            Data::Color([255; 4]),
        ));
    }

    // dbg!(view_from_world);
    // dbg!(clip_from_view);
    // dbg!(clip_from_world);
    // dbg!(world_from_clip);

    let ray_from_pixel = |x: u32, y: u32| -> Ray3 {
        let clip_pos = vec3(
            x as f32 / w as f32 * 2.0 - 1.0,
            y as f32 / h as f32 * 2.0 - 1.0,
            0.5,
        );
        let world_pos = world_from_clip.project_point3(clip_pos);
        Ray3::from_origin_dir(eye, (world_pos - eye).normalize())
    };

    let mut smallest_raw_depth_value = u16::MAX;
    let mut largest_raw_depth_value = 0;
    for pixel in depth_image.pixels() {
        smallest_raw_depth_value = pixel.0[0].min(smallest_raw_depth_value);
        largest_raw_depth_value = pixel.0[0].max(largest_raw_depth_value);
    }
    // dbg!(smallest_raw_depth_value, largest_raw_depth_value);
    let smallest_raw_depth_value = smallest_raw_depth_value as f32;
    let largest_raw_depth_value = largest_raw_depth_value as f32;

    // let closest_table_depth =
    //     camera_plane.distance(ray_from_pixel(0, 0).intersects_plane(Plane3::XY));
    let furthest_table_depth =
        camera_plane.distance(ray_from_pixel(0, h - 1).intersects_plane(Plane3::XY));

    // dbg!(closest_table_depth);
    // dbg!(furthest_table_depth);
    // dbg!(closest_leaf_depth);

    let estimate_depth = move |raw_depth_value: f32| -> f32 {
        let regularizer = 1.0 * largest_raw_depth_value;
        remap(
            1.0 / (regularizer + raw_depth_value),
            (1.0 / regularizer)..=(1.0 / (regularizer + largest_raw_depth_value)),
            furthest_table_depth..=closest_leaf_depth,
        )
    };

    let smallest_depth_value = estimate_depth(smallest_raw_depth_value);
    let largest_depth_value = estimate_depth(largest_raw_depth_value);

    // let mut adjusted_depth_from_estimated = BTreeMap::new();
    // for y in 0..h {
    //     let raw_depth_value = depth_image.get_pixel(0, y).0[0] as f32;
    //     let estimated_depth = estimate_depth(raw_depth_value);
    //     let gt_depth = camera_plane.distance(ray_from_pixel(0, y).intersects_plane(Plane3::XY));
    //     adjusted_depth_from_estimated.insert(not_nan(estimated_depth), gt_depth);
    // }

    // let adjust_depth = |estimated: f32| -> f32 {
    //     let lower = adjusted_depth_from_estimated
    //         .range(..not_nan(estimated))
    //         .rev()
    //         .next();
    //     let higher = adjusted_depth_from_estimated
    //         .range(not_nan(estimated)..)
    //         .next();

    //     if let (Some(lower), Some(higher)) = (lower, higher) {
    //         remap(
    //             estimated,
    //             lower.0.into_inner()..=higher.0.into_inner(),
    //             *lower.1..=*higher.1,
    //         )
    //     } else {
    //         let lower = adjusted_depth_from_estimated.iter().next().unwrap();
    //         let higher = adjusted_depth_from_estimated.iter().rev().next().unwrap();
    //         remap(
    //             estimated,
    //             lower.0.into_inner()..=higher.0.into_inner(),
    //             *lower.1..=*higher.1,
    //         )
    //     }
    // };

    let color_from_depth = |depth: f32| {
        // let t = remap_clamp(depth, 0.5..=1.4, 0.0..=1.0);
        let t = remap_clamp(depth, smallest_depth_value..=largest_depth_value, 0.0..=1.0);
        let t = 1.0 - t;
        let x = (t * (color_map.width() - 1) as f32).round() as u32;
        let [r, g, b] = color_map.get_pixel(x, 0).0;
        [r, g, b, 255]
    };

    let mut points = vec![];

    // Ray trace:
    for x in 0..w {
        for y in 0..h {
            let raw_depth_value = depth_image.get_pixel(x, y).0[0] as f32;
            let estimated_depth = estimate_depth(raw_depth_value);
            // let depth = adjust_depth(estimated_depth);
            let depth = estimated_depth;

            let noise = 0.010;
            let depth = depth * (1.0 + noise * random_range(-1.0..=1.0));

            let ray = ray_from_pixel(x, y);
            let pos = ray.intersects_plane(Plane3 {
                normal: camera_plane.normal,
                d: camera_plane.d - depth,
            });

            let color = if false {
                rgba_image.get_pixel(x, y).0
            } else {
                color_from_depth(depth)
            };

            points.push(Point { pos, color });
        }
    }

    log_points(&points, &logger);
}

// fn not_nan(v: f32) -> ordered_float::NotNan<f32> {
//     ordered_float::NotNan::new(v).unwrap()
// }

fn random_range(range: std::ops::RangeInclusive<f32>) -> f32 {
    use rand::Rng as _;
    remap(rand::thread_rng().gen::<f32>(), 0.0..=1.0, range)
}

fn log_points(points: &[Point], logger: &Logger<'_>) {
    let indices = (0..points.len())
        .map(|i| Index::Sequence(i as _))
        .collect_vec();
    let positions = points.iter().map(|p| p.pos.to_array()).collect_vec();
    let colors = points.iter().map(|p| p.color).collect_vec();

    let time_point = time_point([("time", TimeValue::Time(Time::from_seconds_since_epoch(0.0)))]);

    logger.log(data_msg(
        &time_point,
        ObjPathBuilder::from("points") / Index::Placeholder,
        "pos",
        LoggedData::Batch {
            indices: indices.clone(),
            data: DataVec::Vec3(positions),
        },
    ));

    logger.log(data_msg(
        &time_point,
        ObjPathBuilder::from("points") / Index::Placeholder,
        "color",
        LoggedData::Batch {
            indices,
            data: DataVec::Color(colors),
        },
    ));

    logger.log(data_msg(
        &time_point,
        ObjPathBuilder::from("points") / Index::Placeholder,
        "space",
        LoggedData::BatchSplat(Data::Space(ObjPath::from("world"))),
    ));
}
