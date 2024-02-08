//! Logs a `Tensor` archetype for roundtrip checks.

use rerun::{
    archetypes::TextDocument,
    external::{re_log, re_types::components::MediaType},
    RecordingStream,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log("text_document", &TextDocument::new("Hello, TextDocument!"))?;
    rec.log(
        "markdown",
        &TextDocument::new(
            "# Hello\n\
             Markdown with `code`!\n\
             \n\
             A random image:\n\
             \n\
             ![A random image](https://picsum.photos/640/480)",
        )
        .with_media_type(MediaType::markdown()),
    )?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_text_document")?;
    run(&rec, &args)
}
