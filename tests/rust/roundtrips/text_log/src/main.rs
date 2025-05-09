//! Logs a `Tensor` archetype for roundtrip checks.

use rerun::{
    RecordingStream,
    archetypes::TextLog,
    external::{re_log, re_types::components::TextLogLevel},
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log("log", &TextLog::new("No level"))?;
    rec.log(
        "log",
        &TextLog::new("INFO level").with_level(TextLogLevel::INFO),
    )?;
    rec.log("log", &TextLog::new("WILD level").with_level("WILD"))?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_text_log")?;
    run(&rec, &args)
}
