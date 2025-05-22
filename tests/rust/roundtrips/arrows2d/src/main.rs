//! Logs a `Arrows2D` archetype for roundtrip checks.

use rerun::{RecordingStream, archetypes::Arrows2D};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log(
        "arrows2d",
        &Arrows2D::from_vectors([[4.0, 5.0], [40.0, 50.0]])
            .with_origins([[1.0, 2.0], [10.0, 20.0]])
            .with_radii([0.1, 1.0])
            .with_colors([0xAA0000CC, 0x00BB00DD])
            .with_labels(["hello", "friend"])
            .with_class_ids([126, 127]),
    )
    .map_err(Into::into)
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_arrows2d")?;
    run(&rec, &args)
}
