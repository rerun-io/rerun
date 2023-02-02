use std::f32::consts::PI;

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
    Box3D, ColorRGBA, Component, ComponentName, EntityPath, Label, Mesh3D, MeshId, MsgSender,
    Point2D, Quaternion, Radius, RawMesh3D, Rect2D, Rigid3, Session, TimeType, Timeline, Transform,
    Vec3D, ViewCoordinates,
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
