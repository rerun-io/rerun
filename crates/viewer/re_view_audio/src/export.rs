use crate::processing::AudioProcessingSettings;
use crate::visualizer_system::AudioWaveform;

pub fn export_wav(
    waveform: &AudioWaveform,
    enabled_channels: &[usize],
    mixdown: bool,
    processing: &AudioProcessingSettings,
    region_ns: Option<(f64, f64)>,
) -> Result<Vec<u8>, String> {
    let Some(first_chunk) = waveform.chunks.first() else {
        return Err("no audio chunks to export".to_owned());
    };

    let sample_rate = first_chunk.sample_rate;
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        return Err("audio sample rate must be positive".to_owned());
    }

    let channels_count = if mixdown { 1 } else { enabled_channels.len() };
    if channels_count == 0 {
        return Err("enable at least one audio channel".to_owned());
    }

    let (full_start_ns, full_end_ns) = waveform
        .time_range_ns()
        .ok_or_else(|| "audio waveform has no finite time range".to_owned())?;
    let (start_ns, end_ns) = region_ns.unwrap_or((full_start_ns, full_end_ns));
    if end_ns <= start_ns {
        return Err("export region must have a positive duration".to_owned());
    }

    let sample_rate_hz = sample_rate.round() as u32;
    let total_frames = (((end_ns - start_ns) / 1_000_000_000.0) * sample_rate).ceil() as usize;
    let mut samples = vec![0.0_f32; total_frames * channels_count];

    for chunk in &waveform.chunks {
        if (chunk.sample_rate - sample_rate).abs() > f64::EPSILON {
            return Err("export requires all chunks to have the same sample rate".to_owned());
        }

        let processed_channels = enabled_channels
            .iter()
            .filter_map(|channel_idx| chunk.channels.get(*channel_idx))
            .map(|channel| {
                crate::processing::process_samples(channel, chunk.sample_rate, processing)
            })
            .collect::<Vec<_>>();

        if processed_channels.is_empty() {
            continue;
        }

        let chunk_frames = processed_channels
            .iter()
            .map(Vec::len)
            .min()
            .unwrap_or_default();
        let processed_mix = mixdown.then(|| {
            let mut mixed = vec![0.0; chunk_frames];
            for channel in &processed_channels {
                for (sample_idx, sample) in channel.iter().take(chunk_frames).enumerate() {
                    mixed[sample_idx] += sample;
                }
            }
            for sample in &mut mixed {
                *sample /= processed_channels.len() as f64;
            }
            mixed
        });

        for frame_idx in 0..chunk_frames {
            let sample_time_ns =
                chunk.start_time.as_f64() + frame_idx as f64 / sample_rate * 1_000_000_000.0;
            if sample_time_ns < start_ns || sample_time_ns >= end_ns {
                continue;
            }

            let dst_frame =
                (((sample_time_ns - start_ns) / 1_000_000_000.0) * sample_rate).round() as usize;
            if dst_frame >= total_frames {
                continue;
            }

            if let Some(mix) = processed_mix.as_ref() {
                samples[dst_frame] = mix
                    .get(frame_idx)
                    .copied()
                    .unwrap_or_default()
                    .clamp(-1.0, 1.0) as f32;
            } else {
                for (out_channel_idx, channel) in processed_channels.iter().enumerate() {
                    samples[dst_frame * channels_count + out_channel_idx] = channel
                        .get(frame_idx)
                        .copied()
                        .unwrap_or_default()
                        .clamp(-1.0, 1.0)
                        as f32;
                }
            }
        }
    }

    Ok(encode_pcm16_wav(
        &samples,
        sample_rate_hz,
        channels_count as u16,
    ))
}

fn encode_pcm16_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let data_bytes = (samples.len() * 2) as u32;
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels * 2;
    let mut wav = Vec::with_capacity(44 + data_bytes as usize);

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());

    for sample in samples {
        let quantized = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        wav.extend_from_slice(&quantized.to_le_bytes());
    }

    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_pcm16_wav_header() {
        let wav = encode_pcm16_wav(&[0.0, 1.0, -1.0], 16_000, 1);

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 3 * 2);
    }
}
