//! Logs a `Tensor` archetype for roundtrip checks.

use rerun::{RecordingStream, archetypes::Tensor};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn run(rec: &RecordingStream, _args: &Args) -> anyhow::Result<()> {
    let tensor = ndarray::Array::from_shape_vec((3, 4, 5, 6), (0..360).collect::<Vec<i32>>())?;

    rec.log("tensor", &Tensor::try_from(tensor)?)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_roundtrip_tensor")?;
    run(&rec, &args)
}
