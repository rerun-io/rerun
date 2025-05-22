//! Logs a `Pinhole` archetype for roundtrip checks.

use rerun::{RecordingStream, archetypes::Pinhole, datatypes::Mat3x3};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log(
        "pinhole",
        &Pinhole::new(Mat3x3::from([[3., 0., 1.5], [0., 3., 1.5], [0., 0., 1.]]))
            .with_resolution([3840., 2160.]),
    )?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_pinhole")?;
    run(&rec, &args)
}
