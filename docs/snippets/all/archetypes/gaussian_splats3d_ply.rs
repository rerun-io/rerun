//! Log a 3D Gaussian Splatting (3DGS) PLY file.

use rerun::external::anyhow;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(path) = args.get(1) else {
        anyhow::bail!("Usage: {} <path_to_splats.ply>", args[0]);
    };

    let rec = rerun::RecordingStreamBuilder::new(
        "rerun_example_gaussian_splats3d_ply",
    )
    .spawn()?;

    // No entity-path prefix, and log the data as temporal (not static):
    rec.log_file_from_path(path, None, false)?;

    Ok(())
}
