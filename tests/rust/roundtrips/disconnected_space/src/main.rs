//! Logs a `DisconnectedSpace` archetype for roundtrip checks.

use rerun::{archetypes::DisconnectedSpace, external::re_log, MsgSender, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    MsgSender::from_archetype("disconnected_space", &DisconnectedSpace::new(true))?.send(rec)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun_example_roundtrip_disconnected_space",
        default_enabled,
        move |rec| {
            run(&rec, &args).unwrap();
        },
    )
}
