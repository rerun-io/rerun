//! Create and log audio annotation spans.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_audio_annotation")
        .spawn()?;

    rec.set_time("time", rerun::Duration::from_secs(0.0));
    rec.log(
        "audio/asr",
        &rerun::AudioAnnotation::new("hello", [0.00, 0.32]),
    )?;
    rec.log(
        "audio/asr",
        &rerun::AudioAnnotation::new("world", [0.34, 0.72]),
    )?;

    Ok(())
}
