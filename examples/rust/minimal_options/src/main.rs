//! Demonstrates how to accept arguments and connect to running rerun servers.
//!
//! Usage:
//! ```
//!  cargo run -p minimal_options -- --help
//! ```

use rerun::demo_util::grid;
use rerun::external::re_log;

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    #[clap(long, default_value = "10")]
    num_points_per_axis: usize,

    #[clap(long, default_value = "10.0")]
    radius: f32,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_minimal_options")?;
    run(&rec, &args)
}

fn run(rec: &rerun::RecordingStream, args: &Args) -> anyhow::Result<()> {
    let points = grid(
        glam::Vec3::splat(-args.radius),
        glam::Vec3::splat(args.radius),
        args.num_points_per_axis,
    );
    let colors = grid(
        glam::Vec3::ZERO,
        glam::Vec3::splat(255.0),
        args.num_points_per_axis,
    )
    .map(|v| rerun::Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));

    rec.set_time_sequence("keyframe", 0);
    rec.log(
        "my_points",
        &rerun::Points3D::new(points)
            .with_colors(colors)
            .with_radii([0.5]),
    )?;

    Ok(())
}
