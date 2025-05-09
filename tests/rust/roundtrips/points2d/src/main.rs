//! Logs a `Points2D` archetype for roundtrip checks.

use rerun::{
    RecordingStream,
    archetypes::{Boxes2D, Points2D},
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log(
        "points2d",
        &Points2D::new([(1.0, 2.0), (3.0, 4.0)])
            .with_radii([0.42, 0.43])
            .with_colors([0xAA0000CC, 0x00BB00DD])
            .with_labels(["hello", "friend"])
            .with_draw_order(300.0)
            .with_class_ids([126, 127])
            .with_keypoint_ids([2, 3]),
    )?;

    // Hack to establish 2D view bounds
    rec.log(
        "rect",
        &Boxes2D::from_mins_and_sizes([(0.0, 0.0)], [(4.0, 6.0)]),
    )?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_points2d")?;
    run(&rec, &args)
}
