//! Logs a `Points3D` archetype for roundtrip checks.

use rerun::{RecordingStream, archetypes::Points3D};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log(
        "points3d",
        &Points3D::new([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
            .with_radii([0.42, 0.43])
            .with_colors([0xAA0000CC, 0x00BB00DD])
            .with_labels(["hello", "friend"])
            .with_class_ids([126, 127])
            .with_keypoint_ids([2, 3]),
    )
    .map_err(Into::into)
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_points3d")?;
    run(&rec, &args)
}
