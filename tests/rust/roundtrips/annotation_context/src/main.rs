//! Logs an `AnnotationContext` archetype for roundtrip checks.

use rerun::{
    archetypes::AnnotationContext,
    datatypes::{ClassDescription, Color, KeypointPair},
    external::re_log,
    MsgSender, RecordingStream,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec_stream: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    MsgSender::from_archetype(
        "annotation_context",
        &AnnotationContext::new([
            (1, "hello").into(),
            ClassDescription {
                info: (2, "world", Color::from_rgb(3, 4, 5)).into(),
                keypoint_annotations: vec![(17, "head").into(), (42, "shoulders").into()],
                keypoint_connections: KeypointPair::vec_from([(1, 2), (3, 4)]),
            },
        ]),
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
        "roundtrip_annotation_context",
        default_enabled,
        move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        },
    )
}
