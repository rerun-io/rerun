//! An amalgamation of various usages of the API with synthetic "data" without any particular focus.
//!
//! It uses a lot of different aspects of the Rerun API in order to test it.
//!
//! Run all tests:
//! ```
//! cargo run -p test_api
//! ```
//!
//! Run specific test:
//! ```
//! cargo run -p test_api -- --test rects
//! ```

use std::{collections::HashSet, f32::consts::TAU};

use itertools::Itertools;
use rerun::{
    archetypes::{SegmentationImage, TextLog},
    external::{re_log, re_types::components::TextLogLevel},
    EntityPath, RecordingStream,
};

// --- Rerun logging ---

fn test_bbox(rec: &RecordingStream) -> anyhow::Result<()> {
    use rerun::{
        archetypes::Transform3D,
        components::{Box3D, Color, Radius, Text},
        datatypes::{Angle, RotationAxisAngle, TranslationRotationScale3D},
    };

    rec.set_time_seconds("sim_time", 0f64);
    // TODO(#2786): Box3D archetype
    rec.log_component_batches(
        "bbox_test/bbox",
        false,
        1,
        [
            &Box3D::new(1.0, 0.5, 0.25) as _,
            &Color::from(0x00FF00FF) as _,
            &Radius(0.005) as _,
            &Text("box/t0".into()) as _,
        ],
    )?;
    rec.log(
        "bbox_test/bbox",
        &Transform3D::new(TranslationRotationScale3D::rigid(
            glam::Vec3::ZERO,
            RotationAxisAngle::new(glam::Vec3::Z, Angle::Degrees(180.0)),
        )),
    )?;

    rec.set_time_seconds("sim_time", 1f64);
    rec.log_component_batches(
        "bbox_test/bbox",
        false,
        1,
        [
            &Box3D::new(1.0, 0.5, 0.25) as _,
            &Color::from_rgb(255, 255, 0) as _,
            &Radius(0.01) as _,
            &Text("box/t1".into()) as _,
        ],
    )?;
    rec.log(
        "bbox_test/bbox",
        &Transform3D::new(TranslationRotationScale3D::rigid(
            [1.0, 0.0, 0.0],
            RotationAxisAngle::new(glam::Vec3::Z, Angle::Degrees(180.0)),
        )),
    )?;

    Ok(())
}

fn test_log_cleared(rec: &RecordingStream) -> anyhow::Result<()> {
    use rerun::components::{Color, Rect2D, Text};

    // TODO(#3023): Cleared archetype
    fn log_cleared(rec: &RecordingStream, ent_path: impl Into<EntityPath>, recursive: bool) {
        use rerun::external::re_log_types::PathOp;
        rec.record_path_op(PathOp::clear(recursive, ent_path.into()));
    }

    rec.set_time_seconds("sim_time", 1f64);
    // TODO(#2786): Rect2D archetype
    rec.log_component_batches(
        "null_test/rect/0",
        false,
        1,
        [
            &Rect2D::from_xywh(5.0, 5.0, 4.0, 4.0) as _,
            &Color::from(0xFF0000FF) as _,
            &Text("Rect1".into()) as _,
        ],
    )?;
    rec.log_component_batches(
        "null_test/rect/1",
        false,
        1,
        [
            &Rect2D::from_xywh(10.0, 5.0, 4.0, 4.0) as _,
            &Color::from(0x00FF00FF) as _,
            &Text("Rect2".into()) as _,
        ],
    )?;

    rec.set_time_seconds("sim_time", 2f64);
    log_cleared(rec, "null_test/rect/0", false);

    rec.set_time_seconds("sim_time", 3f64);
    log_cleared(rec, "null_test/rect", true);

    rec.set_time_seconds("sim_time", 4f64);
    rec.log_component_batches(
        "null_test/rect/0",
        false,
        1,
        [&Rect2D::from_xywh(5.0, 5.0, 4.0, 4.0) as _],
    )?;

    rec.set_time_seconds("sim_time", 5f64);
    rec.log_component_batches(
        "null_test/rect/1",
        false,
        1,
        [&Rect2D::from_xywh(10.0, 5.0, 4.0, 4.0) as _],
    )?;

    Ok(())
}

fn test_3d_points(rec: &RecordingStream) -> anyhow::Result<()> {
    use rerun::{
        archetypes::Points3D,
        components::{Color, Point3D, Radius, Text},
    };

    rec.set_time_seconds("sim_time", 1f64);

    rec.log(
        "3d_points/single_point_unlabeled",
        &Points3D::new([(10.0, 0.0, 0.0)]),
    )?;
    rec.log(
        "3d_points/single_point_labeled",
        &Points3D::new([(0.0, 0.0, 0.0)]).with_labels(["labeled point"]),
    )?;

    fn create_points(
        n: usize,
        x: impl Fn(f32) -> f32,
        y: impl Fn(f32) -> f32,
        z: impl Fn(f32) -> f32,
    ) -> (Vec<Text>, Vec<Point3D>, Vec<Radius>, Vec<Color>) {
        use rand::Rng as _;
        let mut rng = rand::thread_rng();
        itertools::multiunzip((0..n).map(|i| {
            let i = i as f32;
            let t = 1.0 - i / (n - 1) as f32;
            (
                Text(i.to_string().into()),
                Point3D::new(x((i * 0.2).sin()), y((i * 0.2).cos()), z(i)),
                Radius(t * 0.1 + (1.0 - t) * 2.0), // lerp(0.1, 2.0, t)
                Color::from_rgb(rng.gen(), rng.gen(), rng.gen()),
            )
        }))
    }

    let (labels, points, radii, _) =
        create_points(9, |x| x * 5.0, |y| y * 5.0 + 10.0, |z| z * 4.0 - 5.0);
    rec.log(
        "3d_points/spiral_small",
        &Points3D::new(points).with_labels(labels).with_radii(radii),
    )?;

    let (labels, points, _, colors) =
        create_points(100, |x| x * 5.0, |y| y * 5.0 - 10.0, |z| z * 0.4 - 5.0);
    rec.log(
        "3d_points/spiral_big",
        &Points3D::new(points)
            .with_labels(labels)
            .with_colors(colors),
    )?;

    Ok(())
}

// TODO(#2786): Rect2D archetype
fn test_rects(rec: &RecordingStream) -> anyhow::Result<()> {
    use ndarray::prelude::*;
    use ndarray_rand::{rand_distr::Uniform, RandomExt as _};

    use rerun::{
        archetypes::Tensor,
        components::{Color, Rect2D},
    };

    // Add an image
    rec.set_time_seconds("sim_time", 1f64);
    let img = Array::<u8, _>::from_elem((1024, 1024, 3, 1).f(), 128);
    rec.log(
        "rects_test/img",
        &Tensor::try_from(img.as_standard_layout().view())?,
    )?;

    // 20 random rectangles
    let rects_xy = Array::random((20, 2), Uniform::new(0.0, 1.0)) * 1024.0f32;
    let rects_wh = Array::random((20, 2), Uniform::new(0.0, 1.0)) * (1024.0 - &rects_xy + 1.0);
    let rects = ndarray::concatenate(Axis(1), &[rects_xy.view(), rects_wh.view()])?
        .axis_iter(Axis(0))
        .map(|r| Rect2D::from_xywh(r[0], r[1], r[2], r[3]))
        .collect_vec();
    let colors = Array::random((20, 3), Uniform::new(0, 255))
        .axis_iter(Axis(0))
        .map(|c| Color::from_rgb(c[0], c[1], c[2]))
        .collect_vec();

    rec.set_time_seconds("sim_time", 2f64);
    rec.log_component_batches(
        "rects_test/rects",
        false,
        rects.len() as _,
        [&rects as _, &colors as _],
    )?;

    // Clear the rectangles by logging an empty set
    rec.set_time_seconds("sim_time", 3f64);
    rec.log_component_batches("rects_test/rects", false, 0, [&Vec::<Rect2D>::new() as _])?;

    Ok(())
}

fn colored_tensor<F: Fn(usize, usize) -> [u8; 3]>(
    width: usize,
    height: usize,
    pos_to_color: F,
) -> ndarray::Array3<u8> {
    let pos_to_color = &pos_to_color; // lambda borrow workaround.
    ndarray::Array3::from_shape_vec(
        (height, width, 3),
        (0..height)
            .flat_map(|y| (0..width).flat_map(move |x| pos_to_color(x, y)))
            .collect_vec(),
    )
    .unwrap()
}

fn test_2d_layering(rec: &RecordingStream) -> anyhow::Result<()> {
    use ndarray::prelude::*;

    use rerun::{
        archetypes::{Image, LineStrips2D, Points2D},
        components::{DrawOrder, Rect2D},
    };

    rec.set_time_seconds("sim_time", 1f64);

    // Add several overlapping images.
    // Large dark gray in the background
    let img = Array::<u8, _>::from_elem((512, 512, 1).f(), 64)
        .as_standard_layout()
        .view()
        .to_owned();
    rec.log(
        "2d_layering/background",
        &Image::try_from(img)?.with_draw_order(0.0),
    )?;
    // Smaller gradient in the middle
    let img = colored_tensor(256, 256, |x, y| [x as u8, y as u8, 0]);
    rec.log(
        "2d_layering/middle_gradient",
        &Image::try_from(img)?.with_draw_order(1.0),
    )?;
    // Slightly smaller blue in the middle, on the same layer as the previous.
    let img = colored_tensor(192, 192, |_, _| [0, 0, 255]);
    rec.log(
        "2d_layering/middle_blue",
        &Image::try_from(img)?.with_draw_order(1.0),
    )?;
    // Small white on top.
    let img = Array::<u8, _>::from_elem((128, 128, 1).f(), 255);
    rec.log(
        "2d_layering/top",
        &Image::try_from(img)?.with_draw_order(2.0),
    )?;

    // Rectangle in between the top and the middle.
    rec.log_component_batches(
        "2d_layering/rect_between_top_and_middle",
        false,
        1,
        [
            &Rect2D::from_xywh(64.0, 64.0, 256.0, 256.0) as _,
            &DrawOrder(1.5) as _,
        ],
    )?;

    // Lines behind the rectangle.
    rec.log(
        "2d_layering/lines_behind_rect",
        &LineStrips2D::new([(0..20).map(|i| ((i * 20) as f32, (i % 2 * 100 + 100) as f32))])
            .with_draw_order(1.25),
    )?;

    // And some points in front of the rectangle.
    rec.log(
        "2d_layering/points_between_top_and_middle",
        &Points2D::new(
            (0..256).map(|i| (32.0 + (i / 16) as f32 * 16.0, 64.0 + (i % 16) as f32 * 16.0)),
        )
        .with_draw_order(1.51),
    )?;

    Ok(())
}

fn test_segmentation(rec: &RecordingStream) -> anyhow::Result<()> {
    use rerun::{
        archetypes::{AnnotationContext, Points2D},
        datatypes::{self, AnnotationInfo},
    };

    // TODO(cmc): All of these text logs should really be going through `re_log` and automagically
    // fed back into rerun via a `tracing` backend. At the _very_ least we should have a helper
    // available for this.
    // In either case, this raises the question of tracking time at the SDK level, akin to what the
    // python SDK does.
    fn log_info(rec: &RecordingStream, text: &str) -> anyhow::Result<()> {
        rec.log(
            "logs/seg_test_log",
            &TextLog::new(text).with_level(TextLogLevel::INFO),
        )?;
        Ok(())
    }

    // Log an image before we have set up our labels
    use ndarray::prelude::*;
    let mut segmentation_img = Array::<u8, _>::zeros((128, 128).f());
    segmentation_img.slice_mut(s![10..20, 30..50]).fill(13);
    segmentation_img.slice_mut(s![80..100, 60..80]).fill(42);
    segmentation_img.slice_mut(s![20..50, 90..110]).fill(99);

    rec.set_time_seconds("sim_time", 1f64);

    rec.log(
        "seg_test/img",
        &SegmentationImage::try_from(segmentation_img)?,
    )?;

    // Log a bunch of classified 2D points
    rec.log(
        "seg_test/single_point",
        &Points2D::new([(64.0, 64.0)]).with_class_ids([13]),
    )?;
    rec.log(
        "seg_test/single_point_labeled",
        &Points2D::new([(90.0, 50.0)])
            .with_class_ids([13])
            .with_labels(["labeled point"]),
    )?;
    rec.log(
        "seg_test/several_points0",
        &Points2D::new([(20.0, 50.0), (100.0, 70.0), (60.0, 30.0)]).with_class_ids([42]),
    )?;
    rec.log(
        "seg_test/several_points1",
        &Points2D::new([(40.0, 50.0), (120.0, 70.0), (80.0, 30.0)]).with_class_ids([13, 42, 99]),
    )?;
    rec.log(
        "seg_test/many points",
        &Points2D::new(
            (0..25).map(|i| (100.0 + (i / 5) as f32 * 2.0, 100.0 + (i % 5) as f32 * 2.0)),
        )
        .with_class_ids([42]),
    )?;
    log_info(
        rec,
        "no rects, default colored points, a single point has a label",
    )?;

    rec.set_time_seconds("sim_time", 2f64);

    rec.log(
        "seg_test",
        &AnnotationContext::new([(13, "label1"), (42, "label2"), (99, "label3")]),
    )?;
    log_info(
        rec,
        "default colored rects, default colored points, all points except the \
            bottom right clusters have labels",
    )?;

    rec.set_time_seconds("sim_time", 3f64);

    // Log an updated segmentation map with specific colors
    rec.log(
        "seg_test",
        &AnnotationContext::new([
            (13, "label1", datatypes::Color::from(0xFF0000FF)),
            (42, "label2", datatypes::Color::from(0x00FF00FF)),
            (99, "label3", datatypes::Color::from_rgb(0, 0, 255)),
        ]),
    )?;
    log_info(rec, "points/rects with user specified colors")?;

    rec.set_time_seconds("sim_time", 4f64);

    // Log with a mixture of set and unset colors / labels
    rec.log(
        "seg_test",
        &AnnotationContext::new([
            AnnotationInfo {
                id: 13,
                label: None,
                color: Some(datatypes::Color::from(0xFF0000FF)),
            },
            (42, "label2", datatypes::Color::from(0x00FF00FF)).into(),
            (99, "label3").into(),
        ]),
    )?;
    log_info(
        rec,
        "label1 disappears and everything with label3 is now default colored again",
    )?;

    Ok(())
}

fn test_text_logs(rec: &RecordingStream) -> anyhow::Result<()> {
    // TODO(cmc): the python SDK has some magic that glues the standard logger directly into rerun
    // logs; we're gonna need something similar for rust (e.g. `tracing` backend).

    rec.set_time_seconds("sim_time", 0f64);

    rec.log(
        "logs",
        &TextLog::new("Text with explicitly set color").with_color((255, 215, 0)),
    )?;

    rec.log(
        "logs",
        &TextLog::new("this entry has loglevel TRACE").with_level(TextLogLevel::TRACE),
    )?;

    Ok(())
}

fn test_transforms_3d(rec: &RecordingStream) -> anyhow::Result<()> {
    use rerun::{
        archetypes::{LineStrips3D, Points3D, Transform3D},
        components::{Color, Point3D, ViewCoordinates},
        coordinates::SignedAxis3,
        datatypes::{Angle, RotationAxisAngle, TranslationRotationScale3D, Vec3D},
    };

    let sun_to_planet_distance = 6.0;
    let planet_to_moon_distance = 3.0;
    let rotation_speed_planet = 2.0;
    let rotation_speed_moon = 5.0;

    // Planetary motion is typically in the XY plane.
    fn log_coordinate_space(
        rec: &RecordingStream,
        ent_path: impl Into<EntityPath>,
    ) -> anyhow::Result<()> {
        // TODO(#2816): Pinhole archetype
        let view_coords = ViewCoordinates::from_up_and_handedness(
            SignedAxis3::POSITIVE_Z,
            rerun::coordinates::Handedness::Right,
        );
        rec.log_component_batches(
            ent_path,
            true,
            1,
            [&view_coords as _, &Color::from_rgb(255, 215, 0) as _],
        )
        .map_err(Into::into)
    }
    log_coordinate_space(rec, "transforms3d")?;
    log_coordinate_space(rec, "transforms3d/sun")?;
    log_coordinate_space(rec, "transforms3d/sun/planet")?;
    log_coordinate_space(rec, "transforms3d/sun/planet/moon")?;

    rec.set_time_seconds("sim_time", 0f64);

    // All are in the center of their own space:
    fn log_point(
        rec: &RecordingStream,
        ent_path: impl Into<EntityPath>,
        radius: f32,
        color: [u8; 3],
    ) -> anyhow::Result<()> {
        rec.log(
            ent_path,
            &Points3D::new([Point3D::ZERO])
                .with_radii([radius])
                .with_colors([Color::from_rgb(color[0], color[1], color[2])]),
        )
        .map_err(Into::into)
    }
    log_point(rec, "transforms3d/sun", 1.0, [255, 200, 10])?;
    log_point(rec, "transforms3d/sun/planet", 0.4, [40, 80, 200])?;
    log_point(rec, "transforms3d/sun/planet/moon", 0.15, [180, 180, 180])?;

    // "dust" around the "planet" (and inside, don't care)
    // distribution is quadratically higher in the middle
    use rand::Rng as _;
    let mut rng = rand::thread_rng();
    let points = std::iter::from_fn(|| {
        let radius = rng.gen::<f32>() * planet_to_moon_distance * 0.5;
        let angle = rng.gen::<f32>() * TAU;
        let height = rng.gen::<f32>().powf(0.2) * 0.5 - 0.5;
        Some(Point3D::new(
            radius * angle.sin(),
            radius * angle.cos(),
            height,
        ))
    })
    .take(200)
    .collect::<Vec<_>>();
    rec.log(
        "transforms3d/sun/planet/dust",
        &Points3D::new(points)
            .with_radii([0.025])
            .with_colors([Color::from_rgb(80, 80, 80)]),
    )?;

    // paths where the planet & moon move
    let create_path = |distance: f32| {
        LineStrips3D::new([(0..=100).map(|i| {
            let angle = i as f32 * 0.01 * TAU;
            (angle.sin() * distance, angle.cos() * distance, 0.0)
        })])
    };
    rec.log(
        "transforms3d/sun/planet_path",
        &create_path(sun_to_planet_distance),
    )?;
    rec.log(
        "transforms3d/sun/planet/moon_path",
        &create_path(planet_to_moon_distance),
    )?;

    for i in 0..6 * 120 {
        let time = i as f32 / 120.0;

        rec.set_time_seconds("sim_time", Some(time as f64));

        rec.log(
            "transforms3d/sun/planet",
            &Transform3D::new(rerun::datatypes::TranslationRotationScale3D::rigid(
                [
                    (time * rotation_speed_planet).sin() * sun_to_planet_distance,
                    (time * rotation_speed_planet).cos() * sun_to_planet_distance,
                    0.0,
                ],
                RotationAxisAngle::new(glam::Vec3::X, Angle::Degrees(20.0)),
            )),
        )?;

        rec.log(
            "transforms3d/sun/planet/moon",
            &Transform3D::new(
                TranslationRotationScale3D::from(Vec3D::new(
                    (time * rotation_speed_moon).cos() * planet_to_moon_distance,
                    (time * rotation_speed_moon).sin() * planet_to_moon_distance,
                    0.0,
                ))
                .from_parent(),
            ),
        )?;
    }

    Ok(())
}

// --- Init ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
enum Demo {
    #[value(name("bbox"))]
    BoundingBox,

    #[value(name("log_cleared"))]
    LogCleared,

    #[value(name("3d_points"))]
    Points3D,

    #[value(name("rects"))]
    Rects,

    #[value(name("2d_ordering"))]
    TwoDOrdering,

    #[value(name("segmentation"))]
    Segmentation,

    #[value(name("text"))]
    TextLogs,

    #[value(name("transforms_3d"))]
    Transforms3D,
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// Which test should we run? All of them by default.
    #[clap(long, value_enum)]
    test: Option<Vec<Demo>>,
}

fn run(rec: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    use clap::ValueEnum as _;
    let tests: HashSet<Demo> = args.test.as_ref().map_or_else(
        || Demo::value_variants().iter().copied().collect(),
        |tests| tests.iter().cloned().collect(),
    );

    for test in tests {
        match test {
            Demo::BoundingBox => test_bbox(rec)?,
            Demo::LogCleared => test_log_cleared(rec)?,
            Demo::Points3D => test_3d_points(rec)?,
            Demo::Rects => test_rects(rec)?,
            Demo::TwoDOrdering => test_2d_layering(rec)?,
            Demo::Segmentation => test_segmentation(rec)?,
            Demo::TextLogs => test_text_logs(rec)?,
            Demo::Transforms3D => test_transforms_3d(rec)?,
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun
        .clone()
        .run("rerun_example_test_api_rs", default_enabled, move |rec| {
            run(&rec, &args).unwrap();
        })
}
