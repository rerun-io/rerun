//! Log a video file.

use rerun::external::anyhow;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(path) = args.get(1) else {
        anyhow::bail!("Usage: {} <path_to_video.[mp4]>", args[0]);
    };

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_asset_video").spawn()?;

    rec.log("world/video", &rerun::AssetVideo::from_file_path(path)?)?;

    Ok(())
}
