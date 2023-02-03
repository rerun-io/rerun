use std::f32::consts::{PI, TAU};

use anyhow::{bail, Context};
use clap::Parser;
use rerun::{
    external::{
        re_log,
        re_log_types::{
            external::{arrow2, arrow2_convert},
            ApplicationId,
        },
        re_memory::AccountingAllocator,
        re_sdk_comms,
    },
    log_time, Box3D, ColorRGBA, Component, ComponentName, EntityPath, Label, LineStrip3D, Mesh3D,
    MeshId, MsgSender, Point2D, Point3D, Quaternion, Radius, RawMesh3D, Rect2D, Rigid3, Session,
    SignedAxis3, Tensor, TextEntry, Time, TimeInt, TimeType, Timeline, Transform, Vec3D,
    ViewCoordinates,
};

// --- Rerun logging ---

fn demo_bbox(session: &mut Session) -> anyhow::Result<()> {
    let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);

    MsgSender::new("bbox_demo/bbox")
        .with_time(timeline_sim_time, 0)
        .with_component(&[Box3D::new(1.0, 0.5, 0.25)])?
        .with_component(&[Transform::Rigid3(Rigid3 {
            rotation: Quaternion::new(0.0, 0.0, (PI / 4.0).sin(), (PI / 4.0).cos()),
            translation: Vec3D::default(),
        })])?
        .with_component(&[ColorRGBA::from([0, 255, 0, 255])])?
        .with_component(&[Radius(0.01)])?
        .with_component(&[Label("box/t0".to_owned())])?
        .send(session)?;

    MsgSender::new("bbox_demo/bbox")
        .with_time(timeline_sim_time, 1)
        .with_component(&[Box3D::new(1.0, 0.5, 0.25)])?
        .with_component(&[Transform::Rigid3(Rigid3 {
            rotation: Quaternion::new(0.0, 0.0, (PI / 4.0).sin(), (PI / 4.0).cos()),
            translation: Vec3D::new(1.0, 0.0, 0.0),
        })])?
        .with_component(&[ColorRGBA::from([255, 255, 0, 255])])?
        .with_component(&[Radius(0.02)])?
        .with_component(&[Label("box/t1".to_owned())])?
        .send(session)?;

    Ok(())
}

fn demo_extension_components(session: &mut Session) -> anyhow::Result<()> {
    let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);

    // Hack to establish 2d view bounds
    MsgSender::new("extension_components")
        .with_time(timeline_sim_time, 1)
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
        .with_time(timeline_sim_time, 1)
        .with_component(&[Point2D::new(64.0, 64.0)])?
        .with_component(&[ColorRGBA::from([255, 0, 0, 255])])?
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
        .with_time(timeline_sim_time, 1)
        .with_component(&[
            Point2D::new(32.0, 32.0),
            Point2D::new(32.0, 96.0),
            Point2D::new(96.0, 32.0),
            Point2D::new(96.0, 96.0),
        ])?
        .with_splat(ColorRGBA::from([0, 255, 0, 255]))?
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
    let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);

    // TODO(cmc): need abstractions for this
    fn log_cleared(
        session: &mut Session,
        time: (Timeline, impl Into<TimeInt>),
        ent_path: impl Into<EntityPath>,
        recursive: bool,
    ) {
        use rerun::external::re_log_types::PathOp;
        let timepoint = [
            (Timeline::log_time(), Time::now().into()),
            (time.0, time.1.into()),
        ];
        session.send_path_op(&timepoint.into(), PathOp::clear(recursive, ent_path.into()));
    }

    // sim_time = 1
    MsgSender::new("null_demo/rect/0")
        .with_time(timeline_sim_time, 1)
        .with_component(&[Rect2D::from_xywh(5.0, 4.0, 4.0, 4.0)])?
        .with_component(&[ColorRGBA::from([255, 0, 0, 255])])?
        .with_component(&[Label("Rect1".to_owned())])?
        .send(session)?;
    MsgSender::new("null_demo/rect/1")
        .with_time(timeline_sim_time, 1)
        .with_component(&[Rect2D::from_xywh(10.0, 5.0, 4.0, 4.0)])?
        .with_component(&[ColorRGBA::from([0, 255, 0, 255])])?
        .with_component(&[Label("Rect2".to_owned())])?
        .send(session)?;

    // sim_time = 2
    log_cleared(session, (timeline_sim_time, 2), "null_demo/rect/0", false);

    // sim_time = 3
    log_cleared(session, (timeline_sim_time, 3), "null_demo/rect", true);

    // sim_time = 4
    MsgSender::new("null_demo/rect/0")
        .with_time(timeline_sim_time, 4)
        .with_component(&[Rect2D::from_xywh(5.0, 4.0, 4.0, 4.0)])?
        .send(session)?;

    // sim_time = 5
    MsgSender::new("null_demo/rect/1")
        .with_time(timeline_sim_time, 5)
        .with_component(&[Rect2D::from_xywh(10.0, 4.0, 4.0, 4.0)])?
        .send(session)?;

    Ok(())
}

fn demo_3d_points(session: &mut Session) -> anyhow::Result<()> {
    let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);

    MsgSender::new("3d_points/single_point_unlabeled")
        .with_time(timeline_sim_time, 1)
        .with_component(&[Point3D::new(10.0, 0.0, 0.0)])?
        .send(session)?;

    MsgSender::new("3d_points/single_point_labeled")
        .with_time(timeline_sim_time, 1)
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
            let t = i / (n - 1) as f32;
            (
                Label(i.to_string()),
                Point3D::new(x((i * 0.2).sin()), y((i * 0.2).cos()), z(i)),
                Radius(t * 0.1 + (1.0 - t) * 2.0), // lerp(0.1, 2.0, t)
                ColorRGBA::from([rng.gen(), rng.gen(), rng.gen(), 255]),
            )
        }))
    }

    let (labels, points, radii, _) =
        create_points(9, |x| x * 5.0, |y| y * 5.0 + 10.0, |z| z * 4.0 - 5.0);
    MsgSender::new("3d_points/spiral_small")
        .with_time(timeline_sim_time, 1)
        .with_component(&points)?
        .with_component(&labels)?
        .with_component(&radii)?
        .send(session)?;

    let (labels, points, _, colors) =
        create_points(100, |x| x * 5.0, |y| y * 5.0 - 10.0, |z| z * 0.4 - 5.0);
    MsgSender::new("3d_points/spiral_big")
        .with_time(timeline_sim_time, 1)
        .with_component(&points)?
        .with_component(&labels)?
        .with_component(&colors)?
        .send(session)?;

    Ok(())
}

fn demo_rects(session: &mut Session) -> anyhow::Result<()> {
    let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);

    // TODO(cmc): the python SDK has some higher-level logic to make life simpler (and safer) when
    // logging images of all kind: standard (`log_image`), depth (see `log_depth_image`),
    // (`log_segmentation_image`).
    // We're gonna need some of that.

    use ndarray::prelude::*;
    use ndarray_rand::{rand_distr::Uniform, RandomExt as _};

    // Add an image
    let img = Array::<u8, _>::from_elem((1024, 1024, 3).f(), 128);
    MsgSender::new("rects_demo/img")
        .with_time(timeline_sim_time, 1)
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
        .map(|c| ColorRGBA::from([c[0], c[1], c[2], 255]))
        .collect::<Vec<_>>();
    MsgSender::new("rects_demo/rects")
        .with_time(timeline_sim_time, 2)
        .with_component(&rects)?
        .with_component(&colors)?
        .send(session)?;

    // Clear the rectangles by logging an empty set
    // rr.set_time_seconds("sim_time", 3)
    // rr.log_rects("rects_demo/rects", [])
    MsgSender::new("rects_demo/rects")
        .with_time(timeline_sim_time, 3)
        .with_component(&Vec::<Rect2D>::new())?
        .send(session)?;

    Ok(())
}

fn demo_segmentation(session: &mut Session) -> anyhow::Result<()> {
    let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);

    // TODO(cmc): the python SDK has some higher-level logic to make life simpler (and safer) when
    // logging images of all kind: standard (`log_image`), depth (see `log_depth_image`),
    // (`log_segmentation_image`).
    // We're gonna need some of that.

    // TODO: have to bring out the big guns for this one

    Ok(())
}

// TODO(cmc): not working as expected afaict
fn demo_text_logs(session: &mut Session) -> anyhow::Result<()> {
    // TODO(cmc): the python SDK has some magic that glues the standard logger directly into rerun
    // logs; we're gonna need something similar for rust (e.g. `tracing` backend).

    MsgSender::new("logs")
        .with_component(&[TextEntry::new("Text with explicitly set color", None)])?
        .with_component(&[ColorRGBA::from([255, 215, 0, 255])])?
        .send(session)?;

    MsgSender::new("logs")
        .with_component(&[TextEntry::new(
            "this entry has loglevel TRACE",
            "TRACE".to_owned().into(),
        )])?
        .send(session)?;

    Ok(())
}

fn demo_transforms_3d(session: &mut Session) -> anyhow::Result<()> {
    let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);

    let sun_to_planet_distance = 6.0;
    let planet_to_moon_distance = 3.0;
    let rotation_speed_planet = 2.0;
    let rotation_speed_moon = 5.0;

    fn log_coordinate_space(
        session: &mut Session,
        ent_path: impl Into<EntityPath>,
    ) -> anyhow::Result<()> {
        let view_coords = ViewCoordinates::from_up_and_handedness(
            SignedAxis3::POSITIVE_Z,
            rerun::Handedness::Right,
        );
        MsgSender::new(ent_path.into())
            .with_timeless(true)
            .with_component(&[view_coords])?
            .with_component(&[ColorRGBA::from([255, 215, 0, 255])])?
            .send(session)
            .map_err(Into::into)
    }
    // Planetary motion is typically in the XY plane.
    log_coordinate_space(session, "transforms3d")?;
    log_coordinate_space(session, "transforms3d/sun")?;
    log_coordinate_space(session, "transforms3d/sun/planet")?;
    log_coordinate_space(session, "transforms3d/sun/planet/moon")?;

    fn log_point(
        session: &mut Session,
        ent_path: impl Into<EntityPath>,
        radius: f32,
        color: [u8; 3],
    ) -> anyhow::Result<()> {
        let timeline_sim_time = Timeline::new("sim_time", TimeType::Time);
        MsgSender::new(ent_path.into())
            .with_time(timeline_sim_time, 0)
            .with_component(&[Point3D::ZERO])?
            .with_component(&[Radius(radius)])?
            .with_component(&[ColorRGBA::from([color[0], color[1], color[2], 255])])?
            .send(session)
            .map_err(Into::into)
    }
    // All are in the center of their own space:
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
        .with_time(timeline_sim_time, 0)
        .with_component(&points)?
        .with_splat(Radius(0.025))?
        .with_splat(ColorRGBA::from([80, 80, 80, 255]))?
        .send(session)?;

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
        .with_time(timeline_sim_time, 0)
        .with_component(&[create_path(sun_to_planet_distance)])?
        .send(session)?;
    MsgSender::new("transforms3d/sun/planet/moon_path")
        .with_time(timeline_sim_time, 0)
        .with_component(&[create_path(planet_to_moon_distance)])?
        .send(session)?;

    for i in 0..6 * 120 {
        let time = i as f32 / 120.0;

        MsgSender::new("transforms3d/sun/planet")
            .with_time(timeline_sim_time, (time * 1e9) as i64) // TODO
            .with_component(&[Box3D::new(1.0, 0.5, 0.25)])?
            .with_component(&[Transform::Rigid3(Rigid3 {
                rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0), // TODO
                translation: Vec3D::new(
                    (time * rotation_speed_planet).sin() * sun_to_planet_distance,
                    (time * rotation_speed_planet).cos() * sun_to_planet_distance,
                    0.0,
                ),
            })])?
            .send(session)?;

        // TODO: inverse
        MsgSender::new("transforms3d/sun/planet/moon")
            .with_time(timeline_sim_time, (time * 1e9) as i64) // TODO
            .with_component(&[Box3D::new(1.0, 0.5, 0.25)])?
            .with_component(&[Transform::Rigid3(Rigid3 {
                rotation: Quaternion::new(0.0, 0.0, 0.0, 1.0), // TODO
                translation: Vec3D::new(
                    (time * rotation_speed_moon).sin() * planet_to_moon_distance,
                    (time * rotation_speed_moon).cos() * planet_to_moon_distance,
                    0.0,
                ),
            })])?
            .send(session)?;
    }

    Ok(())
}

// --- Init ---

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
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
    /// Connect to an external viewer
    #[clap(long)]
    connect: bool,

    /// External Address
    #[clap(long)]
    addr: Option<String>,

    #[clap(long, value_enum)]
    demo: Option<Vec<Demo>>,
}

// Use MiMalloc as global allocator (because it is fast), wrapped in Rerun's allocation tracker
// so that the rerun viewer can show how much memory it is using when calling `show`.
#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    // Arg-parsing boiler-plate
    let args = Args::parse();
    dbg!(&args);

    let mut session = rerun::Session::new();
    // TODO: not that friendly for such a simple thing
    session.set_application_id(ApplicationId("api_demo_rs".to_owned()), true);

    // Connect if requested
    if args.connect {
        let addr = if let Some(addr) = &args.addr {
            addr.parse()
        } else {
            Ok(re_sdk_comms::default_server_addr())
        };

        match addr {
            Ok(addr) => {
                session.connect(addr);
            }
            Err(err) => {
                bail!("Bad address: {:?}. {err}", args.addr)
            }
        }
    }

    // TODO: handle demo arg

    demo_bbox(&mut session)?;
    demo_extension_components(&mut session)?;
    demo_log_cleared(&mut session)?;
    demo_3d_points(&mut session)?;
    demo_rects(&mut session)?;
    demo_segmentation(&mut session)?;
    demo_text_logs(&mut session)?;
    demo_transforms_3d(&mut session)?;

    // TODO: spawn_and_connect
    // If not connected, show the GUI inline
    if args.connect {
        session.flush();
    } else {
        let log_messages = session.drain_log_messages_buffer();
        if let Err(err) = rerun::viewer::show(log_messages) {
            bail!("Failed to start viewer: {err}");
        }
    }

    Ok(())
}
