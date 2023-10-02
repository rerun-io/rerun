//! Log a simple 3D asset.

use rerun::{
    archetypes::{Asset3D, ViewCoordinates},
    external::anyhow,
    RecordingStreamBuilder,
};

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(path) = args.get(1) else {
        anyhow::bail!("Usage: {} <path_to_asset.[gltf|glb]>", args[0]);
    };

    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_asset3d_simple").memory()?;

    rec.log_timeless("world", &ViewCoordinates::RIGHT_HAND_Z_UP)?; // Set an up-axis
    rec.log("world/asset", &Asset3D::from_file(path)?)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
