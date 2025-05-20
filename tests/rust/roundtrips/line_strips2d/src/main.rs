//! Logs a `LineStrips2D` archetype for roundtrip checks.

use rerun::{
    RecordingStream,
    archetypes::{Boxes2D, LineStrips2D},
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    rec.log(
        "line_strips2d",
        &LineStrips2D::new(points.chunks(2))
            .with_radii([0.42, 0.43])
            .with_colors([0xAA0000CC, 0x00BB00DD])
            .with_labels(["hello", "friend"])
            .with_draw_order(300.0)
            .with_class_ids([126, 127]),
    )?;

    // Hack to establish 2D view bounds
    rec.log(
        "rect",
        &Boxes2D::from_mins_and_sizes([(-10.0, -10.0)], [(20.0, 20.0)]),
    )?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_line_strips2d")?;
    run(&rec, &args)
}
