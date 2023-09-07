//! Logs a `Tensor` archetype for roundtrip checks.

use rerun::{archetypes::TextDocument, external::re_log, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log("text_document", &TextDocument::new("Hello, TextDocument!"))
        .map_err(Into::into)
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun_example_roundtrip_tensor",
        default_enabled,
        move |rec| {
            run(&rec, &args).unwrap();
        },
    )
}
