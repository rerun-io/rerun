use std::f32::consts::PI;

use anyhow::{bail, Context};
use clap::Parser;
use rerun::{
    external::{re_log, re_log_types::ApplicationId, re_memory::AccountingAllocator, re_sdk_comms},
    Box3D, ColorRGBA, EntityPath, Label, Mesh3D, MeshId, MsgSender, Quaternion, Radius, RawMesh3D,
    Rigid3, Session, TimeType, Timeline, Transform, Vec3D, ViewCoordinates,
};

// --- Rerun logging ---

fn demo_bbox(session: &mut Session) -> anyhow::Result<()> {
    // def run_bounding_box() -> None:
    //     rr.set_time_seconds("sim_time", 0)
    //     rr.log_obb(
    //         "bbox_demo/bbox",
    //         half_size=[1.0, 0.5, 0.25],
    //         position=np.array([0.0, 0.0, 0.0]),
    //         rotation_q=np.array([0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
    //         color=[0, 255, 0],
    //         stroke_width=0.01,
    //         label="box/t0",
    //     )

    //     rr.set_time_seconds("sim_time", 1)
    //     rr.log_obb(
    //         "bbox_demo/bbox",
    //         half_size=[1.0, 0.5, 0.25],
    //         position=np.array([1.0, 0.0, 0.0]),
    //         rotation_q=np.array([0, 0, np.sin(np.pi / 4), np.cos(np.pi / 4)]),
    //         color=[255, 255, 0],
    //         stroke_width=0.02,
    //         label="box/t1",
    //     )

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
