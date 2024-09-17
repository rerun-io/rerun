//! Log a video asset using manually created frame references.
//! TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

use rerun::{external::anyhow, TimeColumn};

fn main() -> anyhow::Result<()> {
    let args = _args;
    let Some(path) = args.get(1) else {
        // TODO(#7354): Only mp4 is supported for now.
        anyhow::bail!("Usage: {} <path_to_video.[mp4]>", args[0]);
    };

    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_asset_video_manual_frames").spawn()?;

    // Log video asset which is referred to by frame references.
    rec.set_time_seconds("video_time", 0.0); // Make sure it's available on the timeline used for the frame references.
    rec.log("video", &rerun::AssetVideo::from_file_path(path)?)?;

    // Send frame references for every 0.1 seconds over a total of 10 seconds.
    // Naturally, this will result in a choppy playback and only makes sense if the video is 10 seconds or longer.
    // TODO(#7368): Point to example using `send_video_frames`.
    //
    // Use `send_columns` to send all frame references in a single call.
    let times = (0..(10 * 10)).map(|t| t as f64 * 0.1).collect::<Vec<_>>();
    let time_column = TimeColumn::new_seconds("video_time", times.iter().copied());
    let frame_reference_indicators =
        <rerun::VideoFrameReference as rerun::Archetype>::Indicator::new_array(times.len());
    let video_timestamps = times
        .into_iter()
        .map(rerun::components::VideoTimestamp::from_seconds)
        .collect::<Vec<_>>();
    rec.send_columns(
        "video",
        [time_column],
        [&frame_reference_indicators as _, &video_timestamps as _],
    )?;

    Ok(())
}
