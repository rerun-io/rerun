//! Logs a `Arrows3D` archetype for roundtrip checks.

use rerun::{archetypes::Arrows3D, external::re_log, MsgSender, RecordingStream};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec_stream: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    MsgSender::from_archetype(
        "arrows3d",
        &Arrows3D::new([[4.0, 5.0, 6.0], [40.0, 50.0, 60.0]])
            .with_origins([[1.0, 2.0, 3.0], [10.0, 20.0, 30.0]])
            .with_radii([0.1, 1.0])
            .with_colors([0xAA0000CC, 0x00BB00DD])
            .with_labels(["hello", "friend"])
            .with_class_ids([126, 127])
            .with_instance_keys([66, 666]),
    )?
    .send(rec_stream)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun_example_roundtrip_arrows3d",
        default_enabled,
        move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        },
    )
}
