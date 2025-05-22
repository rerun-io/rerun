//! Logs a `LineStrips3D` archetype for roundtrip checks.

use rerun::{RecordingStream, archetypes::LineStrips3D};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let points = [[0., 0., 0.], [2., 1., -1.], [4., -1., 3.], [6., 0., 1.5]];
    rec.log(
        "line_strips3d",
        &LineStrips3D::new(points.chunks(2))
            .with_radii([0.42, 0.43])
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

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_line_strips3d")?;
    run(&rec, &args)
}
