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

use itertools::Itertools as _;
use rerun::{
    EntityPath, RecordingStream, TransformRelation,
    archetypes::{Clear, SegmentationImage, TextLog},
    datatypes::Quaternion,
    external::{re_log, re_types::components::TextLogLevel},
};

// --- Rerun logging ---

fn test_bbox(rec: &RecordingStream) -> anyhow::Result<()> {
    use rerun::{archetypes::Boxes3D, components::Color};

    rec.set_duration_secs("sim_time", 0f64);
    rec.log(
        "bbox_test/bbox",
        &Boxes3D::from_half_sizes([(1.0, 0.5, 0.25)])
            .with_colors([0x00FF00FF])
            .with_quaternions([Quaternion::from_xyzw([
                0.0,
                0.0,
                (TAU / 8.0).sin(),
                (TAU / 8.0).cos(),
            ])])
            .with_radii([0.005])
            .with_labels(["box/t0"]),
    )?;
    rec.set_duration_secs("sim_time", 1f64);

    rec.log(
        "bbox_test/bbox",
        &Boxes3D::from_centers_and_half_sizes([(1.0, 0.0, 0.0)], [(1.0, 0.5, 0.25)])
            .with_colors([Color::from_rgb(255, 255, 0)])
            .with_quaternions([Quaternion::from_xyzw([
                0.0,
                0.0,
                (TAU / 8.0).sin(),
                (TAU / 8.0).cos(),
            ])])
            .with_radii([0.01])
            .with_labels(["box/t1"]),
    )?;

    Ok(())
}

fn test_log_cleared(rec: &RecordingStream) -> anyhow::Result<()> {
    use rerun::archetypes::Boxes2D;

    rec.set_duration_secs("sim_time", 1f64);
    rec.log(
        "null_test/rect/0",
        &Boxes2D::from_mins_and_sizes([(5.0, 5.0)], [(4.0, 4.0)])
            .with_colors([0xFF0000FF])
            .with_labels(["Rect1"]),
    )?;
    rec.log(
        "null_test/rect/1",
        &Boxes2D::from_mins_and_sizes([(10.0, 5.0)], [(4.0, 4.0)])
            .with_colors([0x00FF00FF])
            .with_labels(["Rect2"]),
    )?;

    rec.set_duration_secs("sim_time", 2f64);
    rec.log("null_test/rect/0", &Clear::flat())?;

    rec.set_duration_secs("sim_time", 3f64);
    rec.log("null_test/rect", &Clear::recursive())?;

    rec.set_duration_secs("sim_time", 4f64);
    rec.log(
        "null_test/rect/0",
        &Boxes2D::from_mins_and_sizes([(5.0, 5.0)], [(4.0, 4.0)]),
    )?;

    rec.set_duration_secs("sim_time", 5f64);
    rec.log(
        "null_test/rect/1",
        &Boxes2D::from_mins_and_sizes([(10.0, 5.0)], [(4.0, 4.0)]),
    )?;

    Ok(())
}

fn test_3d_points(rec: &RecordingStream) -> anyhow::Result<()> {
    use rerun::{
        archetypes::Points3D,
        components::{Color, Position3D, Radius, Text},
    };

    rec.set_duration_secs("sim_time", 1f64);

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
    ) -> (Vec<Text>, Vec<Position3D>, Vec<Radius>, Vec<Color>) {
        use rand::Rng as _;
        let mut rng = rand::thread_rng();
        itertools::multiunzip((0..n).map(|i| {
            let i = i as f32;
            let t = 1.0 - i / (n - 1) as f32;
            (
                Text(i.to_string().into()),
                Position3D::new(x((i * 0.2).sin()), y((i * 0.2).cos()), z(i)),
                Radius::from(t * 0.1 + (1.0 - t) * 2.0), // lerp(0.1, 2.0, t)
                Color::from_rgb(rng.r#gen(), rng.r#gen(), rng.r#gen()),
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

fn test_rects(rec: &RecordingStream) -> anyhow::Result<()> {
    use ndarray::prelude::*;
    use ndarray_rand::{RandomExt as _, rand_distr::Uniform};

    use rerun::{
        archetypes::{Boxes2D, Tensor},
        components::Color,
    };

    // Add an image
    rec.set_duration_secs("sim_time", 1f64);
    let img = Array::<u8, _>::from_elem((1024, 1024, 3, 1).f(), 128);
    rec.log(
        "rects_test/img",
        &Tensor::try_from(img.as_standard_layout().view())?,
    )?;

    // 20 random rectangles
    let rects_xy = Array::random((20, 2), Uniform::new(0.0, 1.0)) * 1024.0f32;
    let rects_wh = Array::random((20, 2), Uniform::new(0.0, 1.0)) * (1024.0 - &rects_xy + 1.0);
    let colors = Array::random((20, 3), Uniform::new(0, 255))
        .axis_iter(Axis(0))
        .map(|c| Color::from_rgb(c[0], c[1], c[2]))
        .collect_vec();

    rec.set_duration_secs("sim_time", 2f64);
    rec.log(
        "rects_test/rects",
        &Boxes2D::from_mins_and_sizes(
            rects_xy.axis_iter(Axis(0)).map(|v| (v[0], v[1])),
            rects_wh.axis_iter(Axis(0)).map(|v| (v[0], v[1])),
        )
        .with_colors(colors),
    )?;

    // Clear the rectangles by logging an empty set
    rec.set_duration_secs("sim_time", 3f64);
    rec.log("rects_test/rects", &Boxes2D::clear_fields())?;

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

    rec.set_duration_secs("sim_time", 1f64);

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
        "seg_test/many_points",
        &Points2D::new(
            (0..25).map(|i| (100.0 + (i / 5) as f32 * 2.0, 100.0 + (i % 5) as f32 * 2.0)),
        )
        .with_class_ids([42]),
    )?;
    log_info(
        rec,
        "no rects, default colored points, a single point has a label",
    )?;

    rec.set_duration_secs("sim_time", 2f64);

    rec.log(
        "seg_test",
        &AnnotationContext::new([(13, "label1"), (42, "label2"), (99, "label3")]),
    )?;
    log_info(
        rec,
        "default colored rects, default colored points, all points except the \
            bottom right clusters have labels",
    )?;

    rec.set_duration_secs("sim_time", 3f64);

    // Log an updated segmentation map with specific colors
    rec.log(
        "seg_test",
        &AnnotationContext::new([
            (13, "label1", datatypes::Rgba32::from(0xFF0000FF)),
            (42, "label2", datatypes::Rgba32::from(0x00FF00FF)),
            (99, "label3", datatypes::Rgba32::from_rgb(0, 0, 255)),
        ]),
    )?;
    log_info(rec, "points/rects with user specified colors")?;

    rec.set_duration_secs("sim_time", 4f64);

    // Log with a mixture of set and unset colors / labels
    rec.log(
        "seg_test",
        &AnnotationContext::new([
            AnnotationInfo {
                id: 13,
                label: None,
                color: Some(datatypes::Rgba32::from(0xFF0000FF)),
            },
            (42, "label2", datatypes::Rgba32::from(0x00FF00FF)).into(),
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

    rec.set_duration_secs("sim_time", 0f64);

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
        archetypes::{LineStrips3D, Points3D, Transform3D, ViewCoordinates},
        components::{Color, Position3D},
        datatypes::{Angle, RotationAxisAngle},
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
        rec.log_static(ent_path, &ViewCoordinates::RIGHT_HAND_Z_UP())
            .map_err(Into::into)
    }
    log_coordinate_space(rec, "transforms3d")?;
    log_coordinate_space(rec, "transforms3d/sun")?;
    log_coordinate_space(rec, "transforms3d/sun/planet")?;
    log_coordinate_space(rec, "transforms3d/sun/planet/moon")?;

    rec.set_duration_secs("sim_time", 0f64);

    // All are in the center of their own space:
    fn log_point(
        rec: &RecordingStream,
        ent_path: impl Into<EntityPath>,
        radius: f32,
        color: [u8; 3],
    ) -> anyhow::Result<()> {
        rec.log(
            ent_path,
            &Points3D::new([Position3D::ZERO])
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
        let radius = rng.r#gen::<f32>() * planet_to_moon_distance * 0.5;
        let angle = rng.r#gen::<f32>() * TAU;
        let height = rng.r#gen::<f32>().powf(0.2) * 0.5 - 0.5;
        Some(Position3D::new(
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

        rec.set_duration_secs("sim_time", time as f64);

        rec.log(
            "transforms3d/sun/planet",
            &Transform3D::from_translation_rotation(
                [
                    (time * rotation_speed_planet).sin() * sun_to_planet_distance,
                    (time * rotation_speed_planet).cos() * sun_to_planet_distance,
                    0.0,
                ],
                RotationAxisAngle::new(glam::Vec3::X, Angle::from_degrees(20.0)),
            ),
        )?;

        rec.log(
            "transforms3d/sun/planet/moon",
            &Transform3D::from_translation([
                (time * rotation_speed_moon).cos() * planet_to_moon_distance,
                (time * rotation_speed_moon).sin() * planet_to_moon_distance,
                0.0,
            ])
            .with_relation(TransformRelation::ChildFromParent),
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
        |tests| tests.iter().copied().collect(),
    );

    for test in tests {
        match test {
            Demo::BoundingBox => test_bbox(rec)?,
            Demo::LogCleared => test_log_cleared(rec)?,
            Demo::Points3D => test_3d_points(rec)?,
            Demo::Rects => test_rects(rec)?,
            Demo::Segmentation => test_segmentation(rec)?,
            Demo::TextLogs => test_text_logs(rec)?,
            Demo::Transforms3D => test_transforms_3d(rec)?,
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_test_api")?;
    run(&rec, &args)
}
