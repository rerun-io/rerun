use clap::Parser;

use re_log_types::{field_types, msg_bundle::try_build_msg_bundle2, LogMsg, MsgId};
use rerun_sdk as rerun;

// Setup the rerun allocator
use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

/// Create `len` dummy rectangles
fn build_some_rects(len: usize) -> Vec<field_types::Rect2D> {
    (0..len)
        .into_iter()
        .map(|i| field_types::Rect2D {
            x: i as f32,
            y: i as f32,
            w: (i / 2) as f32,
            h: (i / 2) as f32,
        })
        .collect()
}

/// Create `len` dummy colors
fn build_some_colors(len: usize) -> Vec<field_types::ColorRGBA> {
    (0..len)
        .into_iter()
        .map(|i| field_types::ColorRGBA(i as u32))
        .collect()
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

    // Capture the log_time and object_path
    let time_point = rerun::log_time();
    // Build up some rect data into an arrow array
    let rects = build_some_rects(1);
    let colors = build_some_colors(1);

    let bundle = try_build_msg_bundle2(MsgId::random(), "world/rects", time_point, (rects, colors))
        .ok()
        .unwrap();

    // Create and send one message to the sdk
    let msg = bundle.try_into().unwrap();
    session.send(LogMsg::ArrowMsg(msg));

    // Create and send a second message to the sdk
    for _ in 0..15 {
        let time_point = rerun::log_time();
        let rects = build_some_rects(5);
        let colors = build_some_colors(5);

        let bundle =
            try_build_msg_bundle2(MsgId::random(), "world/rects", time_point, (rects, colors))
                .ok()
                .unwrap();

        let msg = bundle.try_into().unwrap();
        session.send(LogMsg::ArrowMsg(msg));
    }

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
