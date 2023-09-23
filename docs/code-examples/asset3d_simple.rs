//! Log a simple 3D asset.

use rerun::{archetypes::Asset3D, external::anyhow, RecordingStreamBuilder};

fn main() -> Result<(), anyhow::Error> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(path) = args.get(1) else {
        anyhow::bail!("Usage: {} <path_to_asset.[gltf|glb]>", args[0]);
    };

    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_asset3d_simple").memory()?;

    // TODO(#2816): some viewcoords would be nice here
    rec.log("asset", &Asset3D::from_file(path)?)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
