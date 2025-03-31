//! Log a video asset using manually created frame references.
//! TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

use rerun::external::anyhow;

fn main() -> anyhow::Result<()> {
    let args = _args;
    let Some(path) = args.get(1) else {
        // TODO(#7354): Only mp4 is supported for now.
        anyhow::bail!("Usage: {} <path_to_video.[mp4]>", args[0]);
    };

    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_asset_video_manual_frames").spawn()?;

    // Log video asset which is referred to by frame references.
    rec.log_static("video_asset", &rerun::AssetVideo::from_file_path(path)?)?;

    // Create two entities, showing the same video frozen at different times.
    rec.log(
        "frame_1s",
        &rerun::VideoFrameReference::new(rerun::components::VideoTimestamp::from_secs(1.0))
            .with_video_reference("video_asset"),
    )?;
    rec.log(
        "frame_2s",
        &rerun::VideoFrameReference::new(rerun::components::VideoTimestamp::from_secs(2.0))
            .with_video_reference("video_asset"),
    )?;

    // TODO(#5520): log blueprint once supported
    Ok(())
}
