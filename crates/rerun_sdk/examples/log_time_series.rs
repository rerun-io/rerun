use std::f64::consts::TAU;

use clap::Parser;

use re_log_types::{
    field_types::{ColorRGBA, Label, Radius, Scalar, ScalarPlotProps},
    msg_bundle::MsgBundle,
    LogMsg, MsgId, ObjPath, Time, TimePoint, TimeType, Timeline,
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

    log_parabola(&mut session);
    log_trig(&mut session);
    log_segmentation(&mut session);

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

/// Logs a parabola as a time-series.
fn log_parabola(session: &mut Session) {
    let path = ObjPath::from("parabola");
    let labels = vec!["f(t) = (0.01t - 3)³ + 1".to_owned().into()];

    for t in (0..1000).step_by(10) {
        let timepoint = TimePoint::from([
            (
                Timeline::new("log_time", TimeType::Time),
                Time::now().into(),
            ), //
            (Timeline::new("frame_nr", TimeType::Sequence), t.into()),
        ]);

        let t = t as f64;
        let f_of_t = (t * 0.01 - 5.0).powi(3) + 1.0;

        let radius = (f_of_t.abs() * 0.1).clamp(0.5, 10.0) as f32;
        let color = match f_of_t {
            v if v < -10.0 => 0xFF_00_00_FF,
            v if v > 10.0 => 0x00_FF_00_FF,
            _ => 0xFF_FF_00_FF,
        };

        log_scalars(
            session,
            timepoint,
            &path,
            Some(vec![f_of_t.into()]),
            Some(vec![color.into()]),
            Some(vec![radius.into()]),
            Some(labels.clone()),
            None,
        );
    }
}

/// Logs basic trig functions as a time-series.
fn log_trig(session: &mut Session) {
    let cos_path = ObjPath::from("trig/cos");
    let cos_labels = vec!["cos(0.01t)".to_owned().into()];
    let cos_colors = vec![0x00_FF_00_FF.into()];

    let sin_path = ObjPath::from("trig/sin");
    let sin_labels = vec!["sin(0.01t)".to_owned().into()];
    let sin_colors = vec![0xFF_00_00_FF.into()];

    for t in 0..(TAU * 2.0 * 100.0) as i64 {
        let timepoint = TimePoint::from([
            (
                Timeline::new("log_time", TimeType::Time),
                Time::now().into(),
            ), //
            (Timeline::new("frame_nr", TimeType::Sequence), t.into()),
        ]);

        let t = t as f64 * 0.01;

        let sin_of_t = t.sin();
        log_scalars(
            session,
            timepoint.clone(),
            &sin_path,
            Some(vec![sin_of_t.into()]),
            Some(sin_colors.clone()),
            None,
            Some(sin_labels.clone()),
            None,
        );

        let cos_of_t = t.cos();
        log_scalars(
            session,
            timepoint,
            &cos_path,
            Some(vec![cos_of_t.into()]),
            Some(cos_colors.clone()),
            None,
            Some(cos_labels.clone()),
            None,
        );
    }
}

/// Logs a basic example of segmentation as a time-series.
fn log_segmentation(session: &mut Session) {
    use rand::Rng as _;
    let mut rng = rand::thread_rng();

    let line_path = ObjPath::from("segmentation/line");
    let line_colors = vec![0xFF_FF_00_FF.into()];
    let line_radii = vec![3.0.into()];

    let samples_path = ObjPath::from("segmentation/samples");
    let samples_props = vec![ScalarPlotProps { scattered: true }];

    for t in (0..1_000).step_by(2) {
        let timepoint = TimePoint::from([
            (
                Timeline::new("log_time", TimeType::Time),
                Time::now().into(),
            ), //
            (Timeline::new("frame_nr", TimeType::Sequence), t.into()),
        ]);

        let t = t as f64;

        let f_of_t = (2.0 * 0.01 * t) + 2.0;
        log_scalars(
            session,
            timepoint.clone(),
            &line_path,
            Some(vec![f_of_t.into()]),
            Some(line_colors.clone()),
            Some(line_radii.clone()),
            None,
            None,
        );

        let g_of_t = f_of_t + rng.gen::<f64>() * 10.0 - 5.0;
        let radius = (g_of_t - f_of_t).abs() as f32;
        let color = match g_of_t {
            v if v < f_of_t - 1.5 => 0xFF_00_00_FF,
            v if v > f_of_t + 1.5 => 0x00_FF_00_FF,
            _ => 0xFF_FF_FF_FF,
        };
        log_scalars(
            session,
            timepoint,
            &samples_path,
            Some(vec![g_of_t.into()]),
            Some(vec![color.into()]),
            Some(vec![radius.into()]),
            None,
            Some(samples_props.clone()),
        );
    }
}

// ---

/// Logs a collection of scalars.
//
// TODO(cmc): Make this fancier and move into the SDK
// TODO(cmc): we really, really need batched insertions for plots
#[allow(clippy::too_many_arguments)]
fn log_scalars(
    session: &mut Session,
    timepoint: TimePoint,
    obj_path: &ObjPath,
    scalars: Option<Vec<Scalar>>,
    colors: Option<Vec<ColorRGBA>>,
    radii: Option<Vec<Radius>>,
    labels: Option<Vec<Label>>,
    props: Option<Vec<ScalarPlotProps>>,
) {
    let bundle = MsgBundle::new(
        MsgId::random(),
        obj_path.clone(),
        timepoint,
        [
            scalars.map(|te| te.try_into().unwrap()),
            colors.map(|colors| colors.try_into().unwrap()),
            radii.map(|radii| radii.try_into().unwrap()),
            labels.map(|labels| labels.try_into().unwrap()),
            props.map(|props| props.try_into().unwrap()),
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
