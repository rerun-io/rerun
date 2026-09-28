use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use crate::{AudioBuffer, ChannelLayout};

/// A short jingle for checking that audio output works: the notes C5, E5, G5 in a row.
///
/// Each note has a smooth envelope so the sound neither clicks nor startles.
pub fn test_sound() -> Arc<AudioBuffer> {
    const SAMPLE_RATE: u32 = 48_000;
    const NOTE_SECS: f32 = 0.12;
    const AMPLITUDE: f32 = 0.3;
    let notes_hz = [note(3), note(7), note(10)];

    let frames_per_note = (SAMPLE_RATE as f32 * NOTE_SECS) as usize;
    let samples = notes_hz
        .iter()
        .flat_map(|&frequency| {
            (0..frames_per_note).map(move |frame| {
                let t = frame as f32 / frames_per_note as f32;
                let envelope = (PI * t).sin();
                let phase = TAU * frequency * (frame as f32 / SAMPLE_RATE as f32);
                AMPLITUDE * envelope * phase.sin()
            })
        })
        .collect();

    Arc::new(AudioBuffer {
        sample_rate: SAMPLE_RATE,
        num_channels: 1,
        layout: ChannelLayout::Unknown,
        samples,
    })
}

/// Frequency of a note in equal temperament, as semitones from A4 at 440 Hz.
fn note(semitones_from_a4: i32) -> f32 {
    440.0 * 2.0_f32.powf(semitones_from_a4 as f32 / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_is_short_quiet_and_starts_and_ends_in_silence() {
        let sound = test_sound();
        assert!((sound.duration_secs() - 0.36).abs() < 1e-3);
        let peak = sound.samples.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(0.2 < peak && peak <= 0.3, "peak was {peak}");
        assert_eq!(sound.samples[0], 0.0);
        assert!(sound.samples[sound.samples.len() - 1].abs() < 1e-2);
    }
}
