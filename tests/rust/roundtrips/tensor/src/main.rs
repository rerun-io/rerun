//! Logs a `Tensor` archetype for roundtrip checks.

use rerun::{
    archetypes::Tensor, datatypes::TensorId, external::re_log, MsgSender, RecordingStream,
};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec_stream: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let tensor = ndarray::Array::from_shape_vec((3, 4, 5, 6), (0..360).collect())?;

    // Need a deterministic id for round-trip tests. Used (10..26)
    let id = TensorId {
        uuid: core::array::from_fn(|i| (i + 10) as u8),
    };

    MsgSender::from_archetype("tensor", &Tensor::try_from(tensor)?.with_id(id))?
        .send(rec_stream)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let default_enabled = true;
    args.rerun.clone().run(
        "rerun_example_roundtrip_tensor",
        default_enabled,
        move |rec_stream| {
            run(&rec_stream, &args).unwrap();
        },
    )
}
