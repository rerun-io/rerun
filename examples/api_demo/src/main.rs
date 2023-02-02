use anyhow::{bail, Context};
use clap::Parser;
use rerun::{
    external::{re_log, re_log_types::ApplicationId, re_memory::AccountingAllocator, re_sdk_comms},
    EntityPath, Mesh3D, MeshId, MsgSender, RawMesh3D, Session, TimeType, Timeline, Transform,
    ViewCoordinates,
};

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
