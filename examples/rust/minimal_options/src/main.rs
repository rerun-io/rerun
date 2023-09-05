//! Demonstrates how to accept arguments and connect to running rerun servers.
//!
//! Usage:
//! ```
//!  cargo run -p minimal_options -- --help
//! ```

use rerun::archetypes::Points3D;
use rerun::components::Color;
use rerun::{external::re_log, RecordingStream};

use rerun::demo_util::grid;

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

fn run(rec: &RecordingStream, args: &Args) -> anyhow::Result<()> {
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
    .map(|v| Color::from_rgb(v.x as u8, v.y as u8, v.z as u8));

    rec.set_time_sequence("keyframe", 0);
    rec.log(
        "my_points",
        &Points3D::new(points).with_colors(colors).with_radii([0.5]),
    )?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun_example_minimal_options",
        default_enabled,
        move |rec| {
            run(&rec, &args).unwrap();
        },
    )
}
