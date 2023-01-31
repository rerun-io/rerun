use clap::Parser;

use re_log_types::{
    component_types::{ColorRGBA, TextEntry},
    msg_bundle::MsgBundle,
    EntityPath, LogMsg, MsgId, Time, TimePoint, TimeType, Timeline,
};
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

    let path = EntityPath::from("my/text/logs");

    // Send a single text entry.
    let text_entries = Some(vec![
        TextEntry::new("catastrophic failure", Some(LogLevel::CRITICAL)), //
    ]);
    log_text_entries(&mut session, &path, text_entries, None);

    // Sending the same text entry twice in a single frame is fine!
    // Both will visible in the UI, as expected.
    let text_entries = Some(vec![
        TextEntry::new("catastrophic failure", Some(LogLevel::CRITICAL)), //
    ]);
    log_text_entries(&mut session, &path, text_entries, None);

    // Send a collection of colors: these will be taken into account by the UI when rendering the
    // any text entries that follow.
    let colors = Some(vec![
        ColorRGBA(0xffffffff),
        ColorRGBA(0xff0000ff),
        ColorRGBA(0x00ff00ff),
        ColorRGBA(0x0000ffff),
    ]);
    log_text_entries(&mut session, &path, None, colors);

    // Send a larger collection of text entries: these will be rendered using the color defined
    // above!
    let text_entries = Some(vec![
        TextEntry::new("catastrophic failure", Some(LogLevel::CRITICAL)), //
        TextEntry::new("not going too well", Some(LogLevel::ERROR)),
        TextEntry::new("somewhat relevant", Some(LogLevel::INFO)),
        TextEntry::new("potentially interesting", Some(LogLevel::DEBUG)),
    ]);
    log_text_entries(&mut session, &path, text_entries, None);

    // Send both text entries and their colors at the same time.
    let text_entries = Some(vec![
        TextEntry::new("very", Some(LogLevel::TRACE)), //
        TextEntry::new("detailed", Some(LogLevel::TRACE)),
        TextEntry::new("information", Some(LogLevel::TRACE)),
    ]);
    let colors = Some(vec![
        ColorRGBA(0xaaaa00ff),
        ColorRGBA(0xaa00aaff),
        ColorRGBA(0x00aaaaff),
    ]);
    log_text_entries(&mut session, &path, text_entries, colors);

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

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::upper_case_acronyms)]
pub enum LogLevel {
    CRITICAL,
    ERROR,
    WARN,
    INFO,
    DEBUG,
    TRACE,
}

impl From<LogLevel> for String {
    fn from(val: LogLevel) -> Self {
        match val {
            LogLevel::CRITICAL => "CRITICAL",
            LogLevel::ERROR => "ERROR",
            LogLevel::WARN => "WARN",
            LogLevel::INFO => "INFO",
            LogLevel::DEBUG => "DEBUG",
            LogLevel::TRACE => "TRACE",
        }
        .to_owned()
    }
}

/// Log a collection of text entries and/or colors at both `Time::now()` and frame #42.
/// TODO(cmc): Make this fancier and move into the SDK
fn log_text_entries(
    session: &mut Session,
    entity_path: &EntityPath,
    text_entries: Option<Vec<TextEntry>>,
    colors: Option<Vec<ColorRGBA>>,
) {
    // Capture the log_time and entity_path
    let time_point = TimePoint::from([
        (Timeline::log_time(), Time::now().into()),
        (Timeline::new("frame_nr", TimeType::Sequence), 42.into()),
    ]);

    let bundle = MsgBundle::new(
        MsgId::random(),
        entity_path.clone(),
        time_point,
        [
            text_entries.map(|te| te.try_into().unwrap()),
            colors.map(|colors| colors.try_into().unwrap()),
        ]
        .into_iter()
        .flatten()
        .collect(),
    );

    println!("Logged {bundle}");

    // Create and send one message to the sdk
    let msg = bundle.try_into().unwrap();
    session.send(LogMsg::ArrowMsg(msg));
}
