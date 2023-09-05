//! Logs an `AnnotationContext` archetype for roundtrip checks.

use rerun::{
    archetypes::AnnotationContext,
    datatypes::{ClassDescription, Color, KeypointPair},
    external::re_log,
    RecordingStream,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    rec.log(
        "annotation_context",
        &AnnotationContext::new([
            (1, "hello").into(),
            ClassDescription {
                info: (2, "world", Color::from_rgb(3, 4, 5)).into(),
                keypoint_annotations: vec![(17, "head").into(), (42, "shoulders").into()],
                keypoint_connections: KeypointPair::vec_from([(1, 2), (3, 4)]),
            },
        ]),
    )
    .map_err(Into::into)
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun_example_roundtrip_annotation_context",
        default_enabled,
        move |rec| {
            run(&rec, &args).unwrap();
        },
    )
}
