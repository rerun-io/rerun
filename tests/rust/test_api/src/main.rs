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
    archetypes::{AnnotationContext, LineStrips2D, LineStrips3D, Points2D, Transform3D},
    components::{
        Box3D, Color, DrawOrder, Label, Point2D, Point3D, Radius, Rect2D, Tensor,
        TensorDataMeaning, TextEntry, ViewCoordinates,
    },
    coordinates::SignedAxis3,
    datatypes::{
        self, Angle, AnnotationInfo, RotationAxisAngle, TranslationRotationScale3D, Vec3D,
    },
    external::{
        re_log, re_log_types,
        re_log_types::external::{arrow2, arrow2_convert},
        re_types,
    },
    EntityPath, LegacyComponent, MsgSender, RecordingStream,
};

// --- Rerun logging ---

fn test_bbox(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    rec_stream.set_time_seconds("sim_time", 0f64);
    MsgSender::new("bbox_test/bbox")
        .with_component(&[Box3D::new(1.0, 0.5, 0.25)])?
        .with_component(&[Color::from_rgb(0, 255, 0)])?
        .with_component(&[Radius(0.005)])?
        .with_component(&[Label("box/t0".into())])?
        .send(rec_stream)?;
    MsgSender::from_archetype(
        "bbox_test/bbox",
        &Transform3D::new(RotationAxisAngle::new(glam::Vec3::Z, Angle::Degrees(180.0))),
    )?
    .send(rec_stream)?;

    rec_stream.set_time_seconds("sim_time", 1f64);
    MsgSender::new("bbox_test/bbox")
        .with_component(&[Box3D::new(1.0, 0.5, 0.25)])?
        .with_component(&[Color::from_rgb(255, 255, 0)])?
        .with_component(&[Radius(0.01)])?
        .with_component(&[Label("box/t1".into())])?
        .send(rec_stream)?;
    MsgSender::from_archetype(
        "bbox_test/bbox",
        &Transform3D::new(RotationAxisAngle::new(
            [1.0, 0.0, 0.0],
            Angle::Degrees(180.0),
        )),
    )?
    .send(rec_stream)?;

    Ok(())
}

fn test_extension_components(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    // Hack to establish 2d view bounds
    rec_stream.set_time_seconds("sim_time", 0f64);
    MsgSender::new("extension_components")
        .with_component(&[Rect2D::from_xywh(0.0, 0.0, 128.0, 128.0)])?
        .send(rec_stream)?;

    // Separate extension component
    // TODO(cmc): not that great to have to dig around for arrow2-* reexports :/
    // TODO(cmc): not that great either to have all that boilerplate just to declare the component
    // name.
    #[derive(
        arrow2_convert::ArrowField,
        arrow2_convert::ArrowDeserialize,
        arrow2_convert::ArrowSerialize,
        Clone,
    )]
    #[arrow_field(transparent)]
    struct Confidence(f32);

    impl LegacyComponent for Confidence {
        fn legacy_name() -> re_log_types::ComponentName {
            "ext.confidence".into()
        }
    }

    re_log_types::component_legacy_shim!(Confidence);

    // Single point with our custom component!
    rec_stream.set_time_seconds("sim_time", 0f64);
    MsgSender::new("extension_components/point")
        .with_component(&[Point2D::new(64.0, 64.0)])?
        .with_component(&[Color::from_rgb(255, 0, 0)])?
        .with_component(&[Confidence(0.9)])?
        .send(rec_stream)?;

    // Batch points with extension

    // Separate extension components
    #[derive(
        arrow2_convert::ArrowField,
        arrow2_convert::ArrowDeserialize,
        arrow2_convert::ArrowSerialize,
        Clone,
    )]
    #[arrow_field(transparent)]
    struct Corner(String);

    impl LegacyComponent for Corner {
        fn legacy_name() -> re_log_types::ComponentName {
            "ext.corner".into()
        }
    }

    re_log_types::component_legacy_shim!(Corner);

    #[derive(
        arrow2_convert::ArrowField,
        arrow2_convert::ArrowDeserialize,
        arrow2_convert::ArrowSerialize,
        Clone,
    )]
    #[arrow_field(transparent)]
    struct Training(bool);

    impl LegacyComponent for Training {
        fn legacy_name() -> re_log_types::ComponentName {
            "ext.training".into()
        }
    }

    re_log_types::component_legacy_shim!(Training);

    rec_stream.set_time_seconds("sim_time", 1f64);
    MsgSender::new("extension_components/points")
        .with_component(&[
            Point2D::new(32.0, 32.0),
            Point2D::new(32.0, 96.0),
            Point2D::new(96.0, 32.0),
            Point2D::new(96.0, 96.0),
        ])?
        .with_splat(Color::from_rgb(0, 255, 0))?
        .with_component(&[
            Corner("upper left".into()),
            Corner("lower left".into()),
            Corner("upper right".into()),
            Corner("lower right".into()),
        ])?
        .with_splat(Training(true))?
        .send(rec_stream)?;

    Ok(())
}

fn test_log_cleared(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    // TODO(cmc): need abstractions for this
    fn log_cleared(rec_stream: &RecordingStream, ent_path: impl Into<EntityPath>, recursive: bool) {
        use rerun::external::re_log_types::PathOp;
        rec_stream.record_path_op(PathOp::clear(recursive, ent_path.into()));
    }

    rec_stream.set_time_seconds("sim_time", 1f64);
    MsgSender::new("null_test/rect/0")
        .with_component(&[Rect2D::from_xywh(5.0, 5.0, 4.0, 4.0)])?
        .with_component(&[Color::from_rgb(255, 0, 0)])?
        .with_component(&[Label("Rect1".into())])?
        .send(rec_stream)?;
    MsgSender::new("null_test/rect/1")
        .with_component(&[Rect2D::from_xywh(10.0, 5.0, 4.0, 4.0)])?
        .with_component(&[Color::from_rgb(0, 255, 0)])?
        .with_component(&[Label("Rect2".into())])?
        .send(rec_stream)?;

    rec_stream.set_time_seconds("sim_time", 2f64);
    log_cleared(rec_stream, "null_test/rect/0", false);

    rec_stream.set_time_seconds("sim_time", 3f64);
    log_cleared(rec_stream, "null_test/rect", true);

    rec_stream.set_time_seconds("sim_time", 4f64);
    MsgSender::new("null_test/rect/0")
        .with_component(&[Rect2D::from_xywh(5.0, 5.0, 4.0, 4.0)])?
        .send(rec_stream)?;

    rec_stream.set_time_seconds("sim_time", 5f64);
    MsgSender::new("null_test/rect/1")
        .with_component(&[Rect2D::from_xywh(10.0, 5.0, 4.0, 4.0)])?
        .send(rec_stream)?;

    Ok(())
}

fn test_3d_points(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    rec_stream.set_time_seconds("sim_time", 1f64);

    MsgSender::new("3d_points/single_point_unlabeled")
        .with_component(&[Point3D::new(10.0, 0.0, 0.0)])?
        .send(rec_stream)?;

    MsgSender::new("3d_points/single_point_labeled")
        .with_component(&[Point3D::new(0.0, 0.0, 0.0)])?
        .with_component(&[Label("labeled point".into())])?
        .send(rec_stream)?;

    fn create_points(
        n: usize,
        x: impl Fn(f32) -> f32,
        y: impl Fn(f32) -> f32,
        z: impl Fn(f32) -> f32,
    ) -> (Vec<Label>, Vec<Point3D>, Vec<Radius>, Vec<Color>) {
        use rand::Rng as _;
        let mut rng = rand::thread_rng();
        itertools::multiunzip((0..n).map(|i| {
            let i = i as f32;
            let t = 1.0 - i / (n - 1) as f32;
            (
                Label(i.to_string().into()),
                Point3D::new(x((i * 0.2).sin()), y((i * 0.2).cos()), z(i)),
                Radius(t * 0.1 + (1.0 - t) * 2.0), // lerp(0.1, 2.0, t)
                Color::from_rgb(rng.gen(), rng.gen(), rng.gen()),
            )
        }))
    }

    let (labels, points, radii, _) =
        create_points(9, |x| x * 5.0, |y| y * 5.0 + 10.0, |z| z * 4.0 - 5.0);
    MsgSender::new("3d_points/spiral_small")
        .with_component(&points)?
        .with_component(&labels)?
        .with_component(&radii)?
        .send(rec_stream)?;

    let (labels, points, _, colors) =
        create_points(100, |x| x * 5.0, |y| y * 5.0 - 10.0, |z| z * 0.4 - 5.0);
    MsgSender::new("3d_points/spiral_big")
        .with_component(&points)?
        .with_component(&labels)?
        .with_component(&colors)?
        .send(rec_stream)?;

    Ok(())
}

fn test_rects(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    use ndarray::prelude::*;
    use ndarray_rand::{rand_distr::Uniform, RandomExt as _};

    // Add an image
    rec_stream.set_time_seconds("sim_time", 1f64);
    let img = Array::<u8, _>::from_elem((1024, 1024, 3, 1).f(), 128);
    MsgSender::new("rects_test/img")
        .with_component(&[Tensor::try_from(img.as_standard_layout().view())?])?
        .send(rec_stream)?;

    // 20 random rectangles
    // TODO(cmc): shouldn't have to collect, need to fix the "must have a ref" thingy
    let rects_xy = Array::random((20, 2), Uniform::new(0.0, 1.0)) * 1024.0f32;
    let rects_wh = Array::random((20, 2), Uniform::new(0.0, 1.0)) * (1024.0 - &rects_xy + 1.0);
    let rects = ndarray::concatenate(Axis(1), &[rects_xy.view(), rects_wh.view()])?
        .axis_iter(Axis(0))
        .map(|r| Rect2D::from_xywh(r[0], r[1], r[2], r[3]))
        .collect::<Vec<_>>();
    let colors = Array::random((20, 3), Uniform::new(0, 255))
        .axis_iter(Axis(0))
        .map(|c| Color::from_rgb(c[0], c[1], c[2]))
        .collect::<Vec<_>>();

    rec_stream.set_time_seconds("sim_time", 2f64);
    MsgSender::new("rects_test/rects")
        .with_component(&rects)?
        .with_component(&colors)?
        .send(rec_stream)?;

    // Clear the rectangles by logging an empty set
    rec_stream.set_time_seconds("sim_time", 3f64);
    MsgSender::new("rects_test/rects")
        .with_component(&Vec::<Rect2D>::new())?
        .send(rec_stream)?;

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

fn test_2d_layering(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    use ndarray::prelude::*;

    rec_stream.set_time_seconds("sim_time", 1f64);

    // Add several overlapping images.
    // Large dark gray in the background
    let img = Array::<u8, _>::from_elem((512, 512, 1).f(), 64);
    MsgSender::new("2d_layering/background")
        .with_component(&[Tensor::try_from(img.as_standard_layout().view())?])?
        .with_component(&[DrawOrder(0.0)])?
        .send(rec_stream)?;
    // Smaller gradient in the middle
    let img = colored_tensor(256, 256, |x, y| [x as u8, y as u8, 0]);
    MsgSender::new("2d_layering/middle_gradient")
        .with_component(&[Tensor::try_from(img.as_standard_layout().view())?])?
        .with_component(&[DrawOrder(1.0)])?
        .send(rec_stream)?;
    // Slightly smaller blue in the middle, on the same layer as the previous.
    let img = colored_tensor(192, 192, |_, _| [0, 0, 255]);
    MsgSender::new("2d_layering/middle_blue")
        .with_component(&[Tensor::try_from(img.as_standard_layout().view())?])?
        .with_component(&[DrawOrder(1.0)])?
        .send(rec_stream)?;
    // Small white on top.
    let img = Array::<u8, _>::from_elem((128, 128, 1).f(), 255);
    MsgSender::new("2d_layering/top")
        .with_component(&[Tensor::try_from(img.as_standard_layout().view())?])?
        .with_component(&[DrawOrder(2.0)])?
        .send(rec_stream)?;

    // Rectangle in between the top and the middle.
    MsgSender::new("2d_layering/rect_between_top_and_middle")
        .with_component(&[Rect2D::from_xywh(64.0, 64.0, 256.0, 256.0)])?
        .with_component(&[DrawOrder(1.5)])?
        .send(rec_stream)?;

    // Lines behind the rectangle.
    MsgSender::from_archetype(
        "2d_layering/lines_behind_rect",
        &LineStrips2D::new([(0..20).map(|i| ((i * 20) as f32, (i % 2 * 100 + 100) as f32))])
            .with_draw_order(1.25),
    )?
    .send(rec_stream)?;

    // And some points in front of the rectangle.
    MsgSender::from_archetype(
        "2d_layering/points_between_top_and_middle",
        &Points2D::new(
            (0..256).map(|i| (32.0 + (i / 16) as f32 * 16.0, 64.0 + (i % 16) as f32 * 16.0)),
        )
        .with_draw_order(1.51),
    )?
    .send(rec_stream)?;

    Ok(())
}

fn test_segmentation(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    // TODO(cmc): All of these text logs should really be going through `re_log` and automagically
    // fed back into rerun via a `tracing` backend. At the _very_ least we should have a helper
    // available for this.
    // In either case, this raises the question of tracking time at the SDK level, akin to what the
    // python SDK does.
    fn log_info(rec_stream: &RecordingStream, text: &str) -> anyhow::Result<()> {
        MsgSender::new("logs/seg_test_log")
            .with_component(&[TextEntry::new(text, Some("INFO".into()))])?
            .send(rec_stream)
            .map_err(Into::into)
    }

    // Log an image before we have set up our labels
    use ndarray::prelude::*;
    let mut segmentation_img = Array::<u8, _>::zeros((128, 128).f());
    segmentation_img.slice_mut(s![10..20, 30..50]).fill(13);
    segmentation_img.slice_mut(s![80..100, 60..80]).fill(42);
    segmentation_img.slice_mut(s![20..50, 90..110]).fill(99);

    rec_stream.set_time_seconds("sim_time", 1f64);

    let mut tensor = Tensor::try_from(segmentation_img.as_standard_layout().view())?;
    tensor.meaning = TensorDataMeaning::ClassId;
    MsgSender::new("seg_test/img")
        .with_component(&[tensor])?
        .send(rec_stream)?;

    // Log a bunch of classified 2D points
    MsgSender::from_archetype(
        "seg_test/single_point",
        &Points2D::new([(64.0, 64.0)]).with_class_ids([13]),
    )?
    .send(rec_stream)?;
    MsgSender::from_archetype(
        "seg_test/single_point_labeled",
        &Points2D::new([(90.0, 50.0)])
            .with_class_ids([13])
            .with_labels(["labeled point"]),
    )?
    .send(rec_stream)?;
    MsgSender::from_archetype(
        "seg_test/several_points0",
        &Points2D::new([(20.0, 50.0), (100.0, 70.0), (60.0, 30.0)]).with_class_ids([42]),
    )?
    .send(rec_stream)?;
    MsgSender::from_archetype(
        "seg_test/several_points1",
        &Points2D::new([(40.0, 50.0), (120.0, 70.0), (80.0, 30.0)]).with_class_ids([13, 42, 99]),
    )?
    .send(rec_stream)?;
    MsgSender::from_archetype(
        "seg_test/many points",
        &Points2D::new(
            (0..25).map(|i| (100.0 + (i / 5) as f32 * 2.0, 100.0 + (i % 5) as f32 * 2.0)),
        )
        .with_class_ids([42]),
    )?
    .send(rec_stream)?;
    log_info(
        rec_stream,
        "no rects, default colored points, a single point has a label",
    )?;

    rec_stream.set_time_seconds("sim_time", 2f64);

    MsgSender::from_archetype(
        "seg_test",
        &AnnotationContext::new([(13, "label1"), (42, "label2"), (99, "label3")]),
    )?
    .send(rec_stream)?;
    log_info(
        rec_stream,
        "default colored rects, default colored points, all points except the \
            bottom right clusters have labels",
    )?;

    rec_stream.set_time_seconds("sim_time", 3f64);

    // Log an updated segmentation map with specific colors
    MsgSender::from_archetype(
        "seg_test",
        &AnnotationContext::new([
            (13, "label1", datatypes::Color::from_rgb(255, 0, 0)),
            (42, "label2", datatypes::Color::from_rgb(0, 255, 0)),
            (99, "label3", datatypes::Color::from_rgb(0, 0, 255)),
        ]),
    )?
    .send(rec_stream)?;
    log_info(rec_stream, "points/rects with user specified colors")?;

    rec_stream.set_time_seconds("sim_time", 4f64);

    // Log with a mixture of set and unset colors / labels
    MsgSender::from_archetype(
        "seg_test",
        &AnnotationContext::new([
            AnnotationInfo {
                id: 13,
                label: None,
                color: Some(datatypes::Color::from_rgb(255, 0, 0)),
            },
            (42, "label2", datatypes::Color::from_rgb(0, 255, 0)).into(),
            (99, "label3").into(),
        ]),
    )?
    .send(rec_stream)?;
    log_info(
        rec_stream,
        "label1 disappears and everything with label3 is now default colored again",
    )?;

    Ok(())
}

fn test_text_logs(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    // TODO(cmc): the python SDK has some magic that glues the standard logger directly into rerun
    // logs; we're gonna need something similar for rust (e.g. `tracing` backend).

    rec_stream.set_time_seconds("sim_time", 0f64);

    MsgSender::new("logs")
        .with_component(&[TextEntry::new("Text with explicitly set color", None)])?
        .with_component(&[Color::from_rgb(255, 215, 0)])?
        .send(rec_stream)?;

    MsgSender::new("logs")
        .with_component(&[TextEntry::new(
            "this entry has loglevel TRACE",
            Some("TRACE".into()),
        )])?
        .send(rec_stream)?;

    Ok(())
}

fn test_transforms_3d(rec_stream: &RecordingStream) -> anyhow::Result<()> {
    let sun_to_planet_distance = 6.0;
    let planet_to_moon_distance = 3.0;
    let rotation_speed_planet = 2.0;
    let rotation_speed_moon = 5.0;

    // Planetary motion is typically in the XY plane.
    fn log_coordinate_space(
        rec_stream: &RecordingStream,
        ent_path: impl Into<EntityPath>,
    ) -> anyhow::Result<()> {
        let view_coords = ViewCoordinates::from_up_and_handedness(
            SignedAxis3::POSITIVE_Z,
            rerun::coordinates::Handedness::Right,
        );
        MsgSender::new(ent_path.into())
            .with_timeless(true)
            .with_component(&[view_coords])?
            .with_component(&[Color::from_rgb(255, 215, 0)])?
            .send(rec_stream)
            .map_err(Into::into)
    }
    log_coordinate_space(rec_stream, "transforms3d")?;
    log_coordinate_space(rec_stream, "transforms3d/sun")?;
    log_coordinate_space(rec_stream, "transforms3d/sun/planet")?;
    log_coordinate_space(rec_stream, "transforms3d/sun/planet/moon")?;

    rec_stream.set_time_seconds("sim_time", 0f64);

    // All are in the center of their own space:
    fn log_point(
        rec_stream: &RecordingStream,
        ent_path: impl Into<EntityPath>,
        radius: f32,
        color: [u8; 3],
    ) -> anyhow::Result<()> {
        MsgSender::new(ent_path.into())
            .with_component(&[Point3D::ZERO])?
            .with_component(&[Radius(radius)])?
            .with_component(&[Color::from_rgb(color[0], color[1], color[2])])?
            .send(rec_stream)
            .map_err(Into::into)
    }
    log_point(rec_stream, "transforms3d/sun", 1.0, [255, 200, 10])?;
    log_point(rec_stream, "transforms3d/sun/planet", 0.4, [40, 80, 200])?;
    log_point(
        rec_stream,
        "transforms3d/sun/planet/moon",
        0.15,
        [180, 180, 180],
    )?;

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
    MsgSender::new("transforms3d/sun/planet/dust")
        .with_component(&points)?
        .with_splat(Radius(0.025))?
        .with_splat(Color::from_rgb(80, 80, 80))?
        .send(rec_stream)?;

    // paths where the planet & moon move
    let create_path = |distance: f32| {
        LineStrips3D::new([(0..=100).map(|i| {
            let angle = i as f32 * 0.01 * TAU;
            (angle.sin() * distance, angle.cos() * distance, 0.0)
        })])
    };
    MsgSender::from_archetype(
        "transforms3d/sun/planet_path",
        &create_path(sun_to_planet_distance),
    )?
    .send(rec_stream)?;
    MsgSender::from_archetype(
        "transforms3d/sun/planet/moon_path",
        &create_path(planet_to_moon_distance),
    )?
    .send(rec_stream)?;

    for i in 0..6 * 120 {
        let time = i as f32 / 120.0;

        rec_stream.set_time_seconds("sim_time", Some(time as f64));

        MsgSender::from_archetype(
            "transforms3d/sun/planet",
            &Transform3D::new(rerun::datatypes::TranslationRotationScale3D::rigid(
                [
                    (time * rotation_speed_planet).sin() * sun_to_planet_distance,
                    (time * rotation_speed_planet).cos() * sun_to_planet_distance,
                    0.0,
                ],
                RotationAxisAngle::new(glam::Vec3::X, Angle::Degrees(20.0)),
            )),
        )?
        .send(rec_stream)?;

        MsgSender::from_archetype(
            "transforms3d/sun/planet/moon",
            &Transform3D::new(
                TranslationRotationScale3D::from(Vec3D::new(
                    (time * rotation_speed_moon).cos() * planet_to_moon_distance,
                    (time * rotation_speed_moon).sin() * planet_to_moon_distance,
                    0.0,
                ))
                .from_parent(),
            ),
        )?
        .send(rec_stream)?;
    }

    Ok(())
}

// --- Init ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
enum Demo {
    #[value(name("bbox"))]
    BoundingBox,

    #[value(name("extension_components"))]
    ExtensionComponents,

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

fn run(rec_stream: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    use clap::ValueEnum as _;
    let tests: HashSet<Demo> = args.test.as_ref().map_or_else(
        || Demo::value_variants().iter().copied().collect(),
        |tests| tests.iter().cloned().collect(),
    );

    for test in tests {
        match test {
            Demo::BoundingBox => test_bbox(rec_stream)?,
            Demo::ExtensionComponents => test_extension_components(rec_stream)?,
            Demo::LogCleared => test_log_cleared(rec_stream)?,
            Demo::Points3D => test_3d_points(rec_stream)?,
            Demo::Rects => test_rects(rec_stream)?,
            Demo::TwoDOrdering => test_2d_layering(rec_stream)?,
            Demo::Segmentation => test_segmentation(rec_stream)?,
            Demo::TextLogs => test_text_logs(rec_stream)?,
            Demo::Transforms3D => test_transforms_3d(rec_stream)?,
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
        .run("test_api_rs", default_enabled, move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        })
}
