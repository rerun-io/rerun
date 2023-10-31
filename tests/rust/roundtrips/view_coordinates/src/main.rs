//! Logs a `ViewCoordinate` archetype for roundtrip checks.

use rerun::{archetypes::ViewCoordinates, external::re_log, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log_timeless("/", &ViewCoordinates::RDF)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args
        .rerun
        .init("rerun_example_roundtrip_view_coordinates")?;
    run(&rec, &args)
}
