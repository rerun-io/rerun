use clap::Parser;

use re_log_types::{field_types::Tensor, msg_bundle::MsgBundle, LogMsg, MsgId, ObjPath};
use rerun::Session;
use rerun_sdk as rerun;

// Setup the rerun allocator
use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// Connect to an external viewer
    #[clap(long)]
    connect: bool,

    /// External Address
    #[clap(long)]
    addr: Option<String>,
}

fn main() -> std::process::ExitCode {
    // Make sure rerun logging goes to stdout
    re_log::set_default_rust_log_env();
    tracing_subscriber::fmt::init();

    let mut session = rerun_sdk::Session::new();

    // Arg-parsing boiler-plate
    let args = Args::parse();

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
                eprintln!("Bad address: {:?}. {:?}", args.addr, err);
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    let path = ObjPath::from("world/tensors");

    // Send a single tensor
    let a = ndarray::array![[0.0, 1.0, 2.0], [0.0, 2.0, 4.0]]
        .try_into()
        .unwrap();
    let tensors = Some(vec![a]);

    // Send a single rect
    log_tensors(&mut session, &path, tensors);

    // Send a larger collection of rects
    /*
    let rects = Some(vec![
        Rect2D::from_xywh(1.0, 1.0, 2.0, 2.0),
        Rect2D::from_xywh(6.0, 4.0, 1.0, 5.0),
        Rect2D::from_xywh(2.0, 2.0, 2.0, 2.0),
        Rect2D::from_xywh(0.0, 7.0, 5.0, 2.0),
    ]);
    log_tensors(&mut session, &path, rects, None);

    // Send a collection of colors
    let colors = Some(vec![
        ColorRGBA(0xffffffff),
        ColorRGBA(0xff0000ff),
        ColorRGBA(0x00ff00ff),
        ColorRGBA(0x0000ffff),
    ]);
    log_tensors(&mut session, &path, None, colors);

    // Send both rects and colors
    let rects = Some(vec![
        Rect2D::from_xywh(2.0, 2.0, 2.0, 2.0),
        Rect2D::from_xywh(4.0, 2.0, 1.0, 1.0),
        Rect2D::from_xywh(2.0, 4.0, 1.0, 1.0),
    ]);
    let colors = Some(vec![
        ColorRGBA(0xaaaa00ff),
        ColorRGBA(0xaa00aaff),
        ColorRGBA(0x00aaaaff),
    ]);
    log_tensors(&mut session, &path, rects, colors);
    */

    // If not connected, show the GUI inline
    if args.connect {
        session.flush();
    } else {
        let log_messages = session.drain_log_messages_buffer();
        if let Err(err) = rerun_sdk::viewer::show(log_messages) {
            eprintln!("Failed to start viewer: {err}");
            return std::process::ExitCode::FAILURE;
        }
    }

    std::process::ExitCode::SUCCESS
}

/// Log a collection of rects and/or colors
/// TODO(jleibs): Make this fancier and move into the SDK
fn log_tensors(session: &mut Session, obj_path: &ObjPath, tensors: Option<Vec<Tensor>>) {
    // Capture the log_time and object_path
    let time_point = rerun::log_time();

    let bundle = MsgBundle::new(
        MsgId::random(),
        obj_path.clone(),
        time_point,
        [tensors.map(|tensors| tensors.try_into().unwrap())]
            .into_iter()
            .flatten()
            .collect(),
    );

    println!("Logged {bundle}");

    // Create and send one message to the sdk
    let msg = bundle.try_into().unwrap();
    session.send(LogMsg::ArrowMsg(msg));
}
