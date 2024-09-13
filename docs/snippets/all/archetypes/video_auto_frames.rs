//! Log a video asset using automatically determined frame references.
//! TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

use rerun::{external::anyhow, TimeColumn};

fn main() -> anyhow::Result<()> {
    let args = _args;
    let Some(path) = args.get(1) else {
        // TODO(#7354): Only mp4 is supported for now.
        anyhow::bail!("Usage: {} <path_to_video.[mp4]>", args[0]);
    };

    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_asset_video_auto_frames").spawn()?;

    // Log video asset which is referred to by frame references.
    rec.set_time_seconds("video_time", 0.0); // Make sure it's available on the timeline used for the frame references.
    let video_asset = rerun::AssetVideo::from_file_path(path)?;
    rec.log("video", &video_asset)?;

    // Send automatically determined video frame timestamps.
    let frame_timestamps_ns = video_asset.read_frame_timestamps_ns()?;
    let video_timestamps_ns = frame_timestamps_ns
        .iter()
        .copied()
        .map(rerun::components::VideoTimestamp::from_nanoseconds)
        .collect::<Vec<_>>();
    let time_column = TimeColumn::new_nanos(
        "video_time",
        // Note timeline values don't have to be the same as the video timestamps.
        frame_timestamps_ns,
    );
    let frame_reference_indicators =
        <rerun::VideoFrameReference as rerun::Archetype>::Indicator::new_array(
            time_column.num_rows(),
        );
    rec.send_columns(
        "video",
        [time_column],
        [&frame_reference_indicators as _, &video_timestamps_ns as _],
    )?;

    Ok(())
}
