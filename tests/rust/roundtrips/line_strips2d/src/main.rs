//! Logs a `LineStrips2D` archetype for roundtrip checks.

use rerun::{
    archetypes::LineStrips2D, components::Rect2D, external::re_log, MsgSender, RecordingStream,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec_stream: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let points = [[0., 0.], [2., 1.], [4., -1.], [6., 0.]];
    MsgSender::from_archetype(
        "line_strips2d",
        &LineStrips2D::new(points.chunks(2))
            .with_radii([0.42, 0.43])
            .with_colors([0xAA0000CC, 0x00BB00DD])
            .with_labels(["hello", "friend"])
            .with_draw_order(300.0)
            .with_class_ids([126, 127])
            .with_instance_keys([66, 666]),
    )?
    .send(rec_stream)?;

    // Hack to establish 2d view bounds
    MsgSender::new("rect")
        .with_component(&[Rect2D::from_xywh(-10.0, -10.0, 20.0, 20.0)])?
        .send(rec_stream)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun-example-roundtrip_line_strips2d",
        default_enabled,
        move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        },
    )
}
