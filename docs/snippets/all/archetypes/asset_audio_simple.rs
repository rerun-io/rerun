//! Log an audio file at t=0 on the `time` timeline.

use rerun::external::anyhow;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(path) = args.get(1) else {
        anyhow::bail!(
            "Usage: {} <path_to_audio.[aac|flac|m4a|mp3|ogg|wav]>",
            args[0]
        );
    };

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_asset_audio")
        .spawn()?;

    rec.set_duration_secs("time", 0.0);
    rec.log("audio", &rerun::AssetAudio::from_file_path(path)?)?;

    Ok(())
}
