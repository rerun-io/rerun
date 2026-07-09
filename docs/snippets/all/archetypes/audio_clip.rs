//! Create and log a multi-channel audio clip.

use ndarray::{Array2, ShapeBuilder as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_audio_clip")
            .spawn()?;

    let sample_rate = 16_000.0;
    let seconds = 2.0;
    let sample_count = (sample_rate * seconds) as usize;

    let samples = Array2::from_shape_fn((sample_count, 2).f(), |(i, channel)| {
        let t = i as f32 / sample_rate as f32;
        let frequency = if channel == 0 { 220.0 } else { 330.0 };
        0.4 * (std::f32::consts::TAU * frequency * t).sin()
    });

    rec.set_time("time", rerun::Duration::from_secs(0.0));
    let samples =
        rerun::datatypes::TensorData::try_from(samples)?.with_dim_names(["sample", "channel"]);

    rec.log(
        "audio",
        &rerun::AudioClip::new(samples, sample_rate).with_channel_names(["left", "right"]),
    )?;

    Ok(())
}
