//! Logs a `DisconnectedSpace` archetype for roundtrip checks.

use rerun::{archetypes::DisconnectedSpace, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log("disconnected_space", &DisconnectedSpace::new(true))
        .map_err(Into::into)
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args
        .rerun
        .init("rerun_example_roundtrip_disconnected_space")?;
    run(&rec, &args)
}
