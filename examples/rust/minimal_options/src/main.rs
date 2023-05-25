//! Demonstrates how to accept arguments and connect to running rerun servers.
//!
//! Usage:
//! ```
//!  cargo run -p minimal_options -- --help
//! ```

use rerun::components::{ColorRGBA, Point3D, Radius};
use rerun::time::{TimeType, Timeline};
use rerun::{external::re_log, MsgSender, RecordingStream};

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

fn run(rec_stream: &RecordingStream, args: &Args) -> anyhow::Result<()> {
    let timeline_keyframe = Timeline::new("keyframe", TimeType::Sequence);

    let points = grid(
        glam::Vec3::splat(-args.radius),
        glam::Vec3::splat(args.radius),
        args.num_points_per_axis,
    )
    .map(Point3D::from)
    .collect::<Vec<_>>();
    let colors = grid(
        glam::Vec3::ZERO,
        glam::Vec3::splat(255.0),
        args.num_points_per_axis,
    )
    .map(|v| ColorRGBA::from_rgb(v.x as u8, v.y as u8, v.z as u8))
    .collect::<Vec<_>>();

    MsgSender::new("my_points")
        .with_component(&points)?
        .with_component(&colors)?
        .with_splat(Radius(0.5))?
        .with_time(timeline_keyframe, 0)
        .send(rec_stream)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun
        .clone()
        .run("minimal_options", default_enabled, move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        })
}
