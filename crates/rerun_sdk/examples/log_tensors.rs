use clap::Parser;

use ndarray_rand::RandomExt;
use re_log_types::{field_types::Tensor, msg_bundle::MsgBundle, EntityPath, LogMsg, MsgId};
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
    re_log::setup_native_logging();

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

    let path = EntityPath::from("world/tensors");

    // Send a single tensor
    let a = ndarray::Array::random((48, 48), ndarray_rand::rand_distr::Uniform::new(0u8, 255u8));
    let tensors = Some(vec![a.try_into().unwrap()]);
    log_tensors(&mut session, &path, tensors);

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
fn log_tensors(session: &mut Session, entity_path: &EntityPath, tensors: Option<Vec<Tensor>>) {
    // Capture the log_time and object_path
    let time_point = rerun::log_time();

    let bundle = MsgBundle::new(
        MsgId::random(),
        entity_path.clone(),
        time_point,
        tensors
            .map(|tensors| vec![tensors.try_into().unwrap()])
            .unwrap_or_default(),
    );

    // Create and send one message to the sdk
    let msg = bundle.try_into().unwrap();
    session.send(LogMsg::ArrowMsg(msg));
}
