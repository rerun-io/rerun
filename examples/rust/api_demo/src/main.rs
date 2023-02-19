//! An amalgamation of various usages of the API with synthetic "data" without any particular focus.
//!
//! It uses a lot of different aspects of the Rerun API in order to test it.
//!
//! Run all demos:
//! ```
//! cargo run -p api_demo
//! ```
//!
//! Run specific demo:
//! ```
//! cargo run -p api_demo -- --demo rects
//! ```

use std::{
    collections::HashSet,
    f32::consts::{PI, TAU},
};

use rerun::{
    components::{
        AnnotationContext, AnnotationInfo, Box3D, ClassDescription, ClassId, ColorRGBA, Label,
        LineStrip3D, Point2D, Point3D, Quaternion, Radius, Rect2D, Rigid3, Tensor,
        TensorDataMeaning, TextEntry, Transform, Vec3D, ViewCoordinates,
    },
    coordinates::SignedAxis3,
    external::{
        re_log,
        re_log_types::external::{arrow2, arrow2_convert},
    },
    time::{Time, TimePoint, TimeType, Timeline},
    Component, ComponentName, EntityPath, MsgSender, Session,
};

// --- Rerun logging ---

fn sim_time(at: f64) -> TimePoint {
    let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);
    let time = Time::from_seconds_since_epoch(at);
    [(timeline_sim_time, time.into())].into()
}

fn demo_bbox(session: &mut Session) -> anyhow::Result<()> {
    MsgSender::new("bbox_demo/bbox")
        .with_timepoint(sim_time(0 as _))
        .with_component(&[Box3D::new(1.0, 0.5, 0.25)])?
        .with_component(&[Transform::Rigid3(Rigid3 {
            rotation: Quaternion::new(0.0, 0.0, (PI / 4.0).sin(), (PI / 4.0).cos()),
            translation: Vec3D::default(),
        })])?
        .with_component(&[ColorRGBA::from_rgb(0, 255, 0)])?
        .with_component(&[Radius(0.005)])?
        .with_component(&[Label("box/t0".to_owned())])?
        .send(session)?;

    MsgSender::new("bbox_demo/bbox")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[Box3D::new(1.0, 0.5, 0.25)])?
        .with_component(&[Transform::Rigid3(Rigid3 {
            rotation: Quaternion::new(0.0, 0.0, (PI / 4.0).sin(), (PI / 4.0).cos()),
            translation: Vec3D::new(1.0, 0.0, 0.0),
        })])?
        .with_component(&[ColorRGBA::from_rgb(255, 255, 0)])?
        .with_component(&[Radius(0.01)])?
        .with_component(&[Label("box/t1".to_owned())])?
        .send(session)?;

    Ok(())
}

fn demo_extension_components(session: &mut Session) -> anyhow::Result<()> {
    // Hack to establish 2d view bounds
    MsgSender::new("extension_components")
        .with_timepoint(sim_time(0 as _))
        .with_component(&[Rect2D::from_xywh(0.0, 0.0, 128.0, 128.0)])?
        .send(session)?;

    // Separate extension component
    // TODO(cmc): not that great to have to dig around for arrow2-* reexports :/
    // TODO(cmc): not that great either to have all that boilerplate just to declare the component
    // name.
    #[derive(arrow2_convert::ArrowField, arrow2_convert::ArrowSerialize)]
    #[arrow_field(transparent)]
    struct Confidence(f32);
    impl Component for Confidence {
        fn name() -> ComponentName {
            "ext.confidence".into()
        }
    }

    // Single point with our custom component!
    MsgSender::new("extension_components/point")
        .with_timepoint(sim_time(0 as _))
        .with_component(&[Point2D::new(64.0, 64.0)])?
        .with_component(&[ColorRGBA::from_rgb(255, 0, 0)])?
        .with_component(&[Confidence(0.9)])?
        .send(session)?;

    // Batch points with extension

    // Separate extension components
    #[derive(arrow2_convert::ArrowField, arrow2_convert::ArrowSerialize)]
    #[arrow_field(transparent)]
    struct Corner(String);
    impl Component for Corner {
        fn name() -> ComponentName {
            "ext.corner".into()
        }
    }
    #[derive(arrow2_convert::ArrowField, arrow2_convert::ArrowSerialize)]
    #[arrow_field(transparent)]
    struct Training(bool);
    impl Component for Training {
        fn name() -> ComponentName {
            "ext.training".into()
        }
    }

    MsgSender::new("extension_components/points")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[
            Point2D::new(32.0, 32.0),
            Point2D::new(32.0, 96.0),
            Point2D::new(96.0, 32.0),
            Point2D::new(96.0, 96.0),
        ])?
        .with_splat(ColorRGBA::from_rgb(0, 255, 0))?
        .with_component(&[
            Corner("upper left".into()),
            Corner("lower left".into()),
            Corner("upper right".into()),
            Corner("lower right".into()),
        ])?
        .with_splat(Training(true))?
        .send(session)?;

    Ok(())
}

fn demo_log_cleared(session: &mut Session) -> anyhow::Result<()> {
    // TODO(cmc): need abstractions for this
    fn log_cleared(
        session: &mut Session,
        timepoint: &TimePoint,
        ent_path: impl Into<EntityPath>,
        recursive: bool,
    ) {
        use rerun::external::re_log_types::PathOp;
        let tp = timepoint.iter().collect::<Vec<_>>();
        let timepoint = [
            (Timeline::log_time(), Time::now().into()),
            (*tp[0].0, *tp[0].1),
        ];
        session.send_path_op(&timepoint.into(), PathOp::clear(recursive, ent_path.into()));
    }

    // sim_time = 1
    MsgSender::new("null_demo/rect/0")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[Rect2D::from_xywh(5.0, 5.0, 4.0, 4.0)])?
        .with_component(&[ColorRGBA::from_rgb(255, 0, 0)])?
        .with_component(&[Label("Rect1".into())])?
        .send(session)?;
    MsgSender::new("null_demo/rect/1")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[Rect2D::from_xywh(10.0, 5.0, 4.0, 4.0)])?
        .with_component(&[ColorRGBA::from_rgb(0, 255, 0)])?
        .with_component(&[Label("Rect2".into())])?
        .send(session)?;

    // sim_time = 2
    log_cleared(session, &sim_time(2 as _), "null_demo/rect/0", false);

    // sim_time = 3
    log_cleared(session, &sim_time(3 as _), "null_demo/rect", true);

    // sim_time = 4
    MsgSender::new("null_demo/rect/0")
        .with_timepoint(sim_time(4 as _))
        .with_component(&[Rect2D::from_xywh(5.0, 5.0, 4.0, 4.0)])?
        .send(session)?;

    // sim_time = 5
    MsgSender::new("null_demo/rect/1")
        .with_timepoint(sim_time(5 as _))
        .with_component(&[Rect2D::from_xywh(10.0, 5.0, 4.0, 4.0)])?
        .send(session)?;

    Ok(())
}

fn demo_3d_points(session: &mut Session) -> anyhow::Result<()> {
    MsgSender::new("3d_points/single_point_unlabeled")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[Point3D::new(10.0, 0.0, 0.0)])?
        .send(session)?;

    MsgSender::new("3d_points/single_point_labeled")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[Point3D::new(0.0, 0.0, 0.0)])?
        .with_component(&[Label("labeled point".to_owned())])?
        .send(session)?;

    fn create_points(
        n: usize,
        x: impl Fn(f32) -> f32,
        y: impl Fn(f32) -> f32,
        z: impl Fn(f32) -> f32,
    ) -> (Vec<Label>, Vec<Point3D>, Vec<Radius>, Vec<ColorRGBA>) {
        use rand::Rng as _;
        let mut rng = rand::thread_rng();
        itertools::multiunzip((0..n).map(|i| {
            let i = i as f32;
            let t = 1.0 - i / (n - 1) as f32;
            (
                Label(i.to_string()),
                Point3D::new(x((i * 0.2).sin()), y((i * 0.2).cos()), z(i)),
                Radius(t * 0.1 + (1.0 - t) * 2.0), // lerp(0.1, 2.0, t)
                ColorRGBA::from_rgb(rng.gen(), rng.gen(), rng.gen()),
            )
        }))
    }

    let (labels, points, radii, _) =
        create_points(9, |x| x * 5.0, |y| y * 5.0 + 10.0, |z| z * 4.0 - 5.0);
    MsgSender::new("3d_points/spiral_small")
        .with_timepoint(sim_time(1 as _))
        .with_component(&points)?
        .with_component(&labels)?
        .with_component(&radii)?
        .send(session)?;

    let (labels, points, _, colors) =
        create_points(100, |x| x * 5.0, |y| y * 5.0 - 10.0, |z| z * 0.4 - 5.0);
    MsgSender::new("3d_points/spiral_big")
        .with_timepoint(sim_time(1 as _))
        .with_component(&points)?
        .with_component(&labels)?
        .with_component(&colors)?
        .send(session)?;

    Ok(())
}

fn demo_rects(session: &mut Session) -> anyhow::Result<()> {
    use ndarray::prelude::*;
    use ndarray_rand::{rand_distr::Uniform, RandomExt as _};

    // Add an image
    let img = Array::<u8, _>::from_elem((1024, 1024, 3).f(), 128);
    MsgSender::new("rects_demo/img")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[Tensor::try_from(img.as_standard_layout().view())?])?
        .send(session)?;

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
        .map(|c| ColorRGBA::from_rgb(c[0], c[1], c[2]))
        .collect::<Vec<_>>();
    MsgSender::new("rects_demo/rects")
        .with_timepoint(sim_time(2 as _))
        .with_component(&rects)?
        .with_component(&colors)?
        .send(session)?;

    // Clear the rectangles by logging an empty set
    MsgSender::new("rects_demo/rects")
        .with_timepoint(sim_time(3 as _))
        .with_component(&Vec::<Rect2D>::new())?
        .send(session)?;

    Ok(())
}

fn demo_segmentation(session: &mut Session) -> anyhow::Result<()> {
    // TODO(cmc): All of these text logs should really be going through `re_log` and automagically
    // fed back into rerun via a `tracing` backend. At the _very_ least we should have a helper
    // available for this.
    // In either case, this raises the question of tracking time at the SDK level, akin to what the
    // python SDK does.
    fn log_info(session: &mut Session, timepoint: TimePoint, text: &str) -> anyhow::Result<()> {
        MsgSender::new("logs/seg_demo_log")
            .with_timepoint(timepoint)
            .with_component(&[TextEntry::new(text, Some("INFO".into()))])?
            .send(session)
            .map_err(Into::into)
    }

    // Log an image before we have set up our labels
    use ndarray::prelude::*;
    let mut segmentation_img = Array::<u8, _>::zeros((128, 128).f());
    segmentation_img.slice_mut(s![10..20, 30..50]).fill(13);
    segmentation_img.slice_mut(s![80..100, 60..80]).fill(42);
    segmentation_img.slice_mut(s![20..50, 90..110]).fill(99);

    let mut tensor = Tensor::try_from(segmentation_img.as_standard_layout().view())?;
    tensor.meaning = TensorDataMeaning::ClassId;
    MsgSender::new("seg_demo/img")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[tensor])?
        .send(session)?;

    // Log a bunch of classified 2D points
    MsgSender::new("seg_demo/single_point")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[Point2D::new(64.0, 64.0)])?
        .with_component(&[ClassId(13)])?
        .send(session)?;
    MsgSender::new("seg_demo/single_point_labeled")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[Point2D::new(90.0, 50.0)])?
        .with_component(&[ClassId(13)])?
        .with_component(&[Label("labeled point".into())])?
        .send(session)?;
    MsgSender::new("seg_demo/several_points0")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[
            Point2D::new(20.0, 50.0),
            Point2D::new(100.0, 70.0),
            Point2D::new(60.0, 30.0),
        ])?
        .with_splat(ClassId(42))?
        .send(session)?;
    MsgSender::new("seg_demo/several_points1")
        .with_timepoint(sim_time(1 as _))
        .with_component(&[
            Point2D::new(40.0, 50.0),
            Point2D::new(120.0, 70.0),
            Point2D::new(80.0, 30.0),
        ])?
        .with_component(&[ClassId(13), ClassId(42), ClassId(99)])?
        .send(session)?;
    MsgSender::new("seg_demo/many points")
        .with_timepoint(sim_time(1 as _))
        .with_component(
            &(0..25)
                .map(|i| Point2D::new(100.0 + (i / 5) as f32 * 2.0, 100.0 + (i % 5) as f32 * 2.0))
                .collect::<Vec<_>>(),
        )?
        .with_splat(ClassId(42))?
        .send(session)?;
    log_info(
        session,
        sim_time(1 as _),
        "no rects, default colored points, a single point has a label",
    )?;

    // Log an initial segmentation map with arbitrary colors
    // TODO(cmc): Gotta provide _MUCH_ better helpers for building out annotations, this is just
    // unapologetically painful
    fn create_class(
        id: u16,
        label: Option<&str>,
        color: Option<[u8; 3]>,
    ) -> (ClassId, ClassDescription) {
        (
            ClassId(id),
            ClassDescription {
                info: AnnotationInfo {
                    id,
                    label: label.map(|label| Label(label.into())),
                    color: color.map(|c| ColorRGBA::from_rgb(c[0], c[1], c[2])),
                },
                ..Default::default()
            },
        )
    }
    MsgSender::new("seg_demo")
        .with_timepoint(sim_time(2 as _))
        .with_component(&[AnnotationContext {
            class_map: [
                create_class(13, "label1".into(), None),
                create_class(42, "label2".into(), None),
                create_class(99, "label3".into(), None),
            ]
            .into_iter()
            .collect(),
        }])?
        .send(session)?;
    log_info(
        session,
        sim_time(2 as _),
        "default colored rects, default colored points, all points except the \
            bottom right clusters have labels",
    )?;

    // Log an updated segmentation map with specific colors
    MsgSender::new("seg_demo")
        .with_timepoint(sim_time(3 as _))
        .with_component(&[AnnotationContext {
            class_map: [
                create_class(13, "label1".into(), [255, 0, 0].into()),
                create_class(42, "label2".into(), [0, 255, 0].into()),
                create_class(99, "label3".into(), [0, 0, 255].into()),
            ]
            .into_iter()
            .collect(),
        }])?
        .send(session)?;
    log_info(
        session,
        sim_time(3 as _),
        "points/rects with user specified colors",
    )?;

    // Log with a mixture of set and unset colors / labels
    MsgSender::new("seg_demo")
        .with_timepoint(sim_time(4 as _))
        .with_component(&[AnnotationContext {
            class_map: [
                create_class(13, None, [255, 0, 0].into()),
                create_class(42, "label2".into(), [0, 255, 0].into()),
                create_class(99, "label3".into(), None),
            ]
            .into_iter()
            .collect(),
        }])?
        .send(session)?;
    log_info(
        session,
        sim_time(4 as _),
        "label1 disappears and everything with label3 is now default colored again",
    )?;

    Ok(())
}

fn demo_text_logs(session: &mut Session) -> anyhow::Result<()> {
    // TODO(cmc): the python SDK has some magic that glues the standard logger directly into rerun
    // logs; we're gonna need something similar for rust (e.g. `tracing` backend).

    MsgSender::new("logs")
        // TODO(cmc): The original api_demo has a sim_time associated with its logs because of the
        // stateful nature of time in the python SDK... This tends to show that we really need the
        // same system for the Rust SDK?
        .with_timepoint(sim_time(0 as _))
        .with_component(&[TextEntry::new("Text with explicitly set color", None)])?
        .with_component(&[ColorRGBA::from_rgb(255, 215, 0)])?
        .send(session)?;

    MsgSender::new("logs")
        .with_timepoint(sim_time(0 as _))
        .with_component(&[TextEntry::new(
            "this entry has loglevel TRACE",
            Some("TRACE".into()),
        )])?
        .send(session)?;

    Ok(())
}

fn demo_transforms_3d(session: &mut Session) -> anyhow::Result<()> {
    let sun_to_planet_distance = 6.0;
    let planet_to_moon_distance = 3.0;
    let rotation_speed_planet = 2.0;
    let rotation_speed_moon = 5.0;

    // Planetary motion is typically in the XY plane.
    fn log_coordinate_space(
        session: &mut Session,
        ent_path: impl Into<EntityPath>,
    ) -> anyhow::Result<()> {
        let view_coords = ViewCoordinates::from_up_and_handedness(
            SignedAxis3::POSITIVE_Z,
            rerun::coordinates::Handedness::Right,
        );
        MsgSender::new(ent_path.into())
            .with_timeless(true)
            .with_component(&[view_coords])?
            .with_component(&[ColorRGBA::from_rgb(255, 215, 0)])?
            .send(session)
            .map_err(Into::into)
    }
    log_coordinate_space(session, "transforms3d")?;
    log_coordinate_space(session, "transforms3d/sun")?;
    log_coordinate_space(session, "transforms3d/sun/planet")?;
    log_coordinate_space(session, "transforms3d/sun/planet/moon")?;

    // All are in the center of their own space:
    fn log_point(
        session: &mut Session,
        ent_path: impl Into<EntityPath>,
        radius: f32,
        color: [u8; 3],
    ) -> anyhow::Result<()> {
        MsgSender::new(ent_path.into())
            .with_timepoint(sim_time(0 as _))
            .with_component(&[Point3D::ZERO])?
            .with_component(&[Radius(radius)])?
            .with_component(&[ColorRGBA::from_rgb(color[0], color[1], color[2])])?
            .send(session)
            .map_err(Into::into)
    }
    log_point(session, "transforms3d/sun", 1.0, [255, 200, 10])?;
    log_point(session, "transforms3d/sun/planet", 0.4, [40, 80, 200])?;
    log_point(
        session,
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
        .with_timepoint(sim_time(0 as _))
        .with_component(&points)?
        .with_splat(Radius(0.025))?
        .with_splat(ColorRGBA::from_rgb(80, 80, 80))?
        .send(session)?;

    // paths where the planet & moon move
    let create_path = |distance: f32| {
        LineStrip3D(
            (0..=100)
                .map(|i| {
                    let angle = i as f32 * 0.01 * TAU;
                    Vec3D::new(angle.sin() * distance, angle.cos() * distance, 0.0)
                })
                .collect(),
        )
    };
    MsgSender::new("transforms3d/sun/planet_path")
        .with_timepoint(sim_time(0 as _))
        .with_component(&[create_path(sun_to_planet_distance)])?
        .send(session)?;
    MsgSender::new("transforms3d/sun/planet/moon_path")
        .with_timepoint(sim_time(0 as _))
        .with_component(&[create_path(planet_to_moon_distance)])?
        .send(session)?;

    for i in 0..6 * 120 {
        let time = i as f32 / 120.0;

        MsgSender::new("transforms3d/sun/planet")
            .with_timepoint(sim_time(time as _))
            .with_component(&[Transform::Rigid3(Rigid3 {
                rotation: Quaternion::from(glam::Quat::from_axis_angle(
                    glam::Vec3::X,
                    20.0f32.to_radians(),
                )),
                translation: Vec3D::new(
                    (time * rotation_speed_planet).sin() * sun_to_planet_distance,
                    (time * rotation_speed_planet).cos() * sun_to_planet_distance,
                    0.0,
                ),
            })])?
            .send(session)?;

        MsgSender::new("transforms3d/sun/planet/moon")
            .with_timepoint(sim_time(time as _))
            .with_component(&[Transform::Rigid3(Rigid3 {
                rotation: Quaternion::default(),
                translation: Vec3D::new(
                    -(time * rotation_speed_moon).cos() * planet_to_moon_distance,
                    -(time * rotation_speed_moon).sin() * planet_to_moon_distance,
                    0.0,
                ),
            })])?
            .send(session)?;
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

    /// Which demo should we run? All of them by default.
    #[clap(long, value_enum)]
    demo: Option<Vec<Demo>>,
}

fn run(session: &mut Session, args: &Args) -> anyhow::Result<()> {
    use clap::ValueEnum as _;
    let demos: HashSet<Demo> = args.demo.as_ref().map_or_else(
        || Demo::value_variants().iter().copied().collect(),
        |demos| demos.iter().cloned().collect(),
    );

    for demo in demos {
        match demo {
            Demo::BoundingBox => demo_bbox(session)?,
            Demo::ExtensionComponents => demo_extension_components(session)?,
            Demo::LogCleared => demo_log_cleared(session)?,
            Demo::Points3D => demo_3d_points(session)?,
            Demo::Rects => demo_rects(session)?,
            Demo::Segmentation => demo_segmentation(session)?,
            Demo::TextLogs => demo_text_logs(session)?,
            Demo::Transforms3D => demo_transforms_3d(session)?,
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let mut session = rerun::Session::init("api_demo_rs", true);

    let should_spawn = args.rerun.on_startup(&mut session);
    if should_spawn {
        return session
            .spawn(move |mut session| run(&mut session, &args))
            .map_err(Into::into);
    }

    run(&mut session, &args)?;

    args.rerun.on_teardown(&mut session)?;

    Ok(())
}
