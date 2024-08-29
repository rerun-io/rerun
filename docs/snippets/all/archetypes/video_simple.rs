//! Log a simple 3D asset.

use rerun::external::anyhow;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(path) = args.get(1) else {
        anyhow::bail!("Usage: {} <path_to_asset.[gltf|glb|obj|stl]>", args[0]);
    };

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_asset_video").spawn()?;

    rec.log("world/video", &rerun::AssetVideo::from_file(path)?)?;

    Ok(())
}
