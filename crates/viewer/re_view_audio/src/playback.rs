use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use re_log_types::{TimeInt, TimeReal};
use tinyaudio::{OutputDevice, OutputDeviceParameters, run_output_device};

use crate::processing::AudioProcessingSettings;
use crate::visualizer_system::AudioWaveform;

pub struct AudioPlayback {
    _device: OutputDevice,
    cursor_frame: Arc<AtomicUsize>,
    start_time_ns: f64,
    sample_rate: f64,
    total_frames: usize,
    loop_start_frame: Option<usize>,
}

impl AudioPlayback {
    pub fn start(
        waveform: &AudioWaveform,
        enabled_channels: &[usize],
        mixdown: bool,
        processing: &AudioProcessingSettings,
        loop_region_ns: Option<(f64, f64)>,
        cursor_time: TimeInt,
    ) -> Result<Self, String> {
        let buffer =
            PlaybackBuffer::from_waveform(waveform, enabled_channels, mixdown, processing)?;
        let loop_start_frame = loop_region_ns.map(|(start, _)| buffer.frame_for_time_ns(start));
        let loop_end_frame = loop_region_ns.map(|(_, end)| buffer.frame_for_time_ns(end));
        let start_frame = loop_start_frame
            .unwrap_or_else(|| buffer.frame_for_time(cursor_time))
            .min(buffer.total_frames());
        let cursor_frame = Arc::new(AtomicUsize::new(start_frame));
        let callback_cursor = Arc::clone(&cursor_frame);
        let total_frames = buffer.total_frames();
        let samples = Arc::new(buffer.samples);
        let callback_samples = Arc::clone(&samples);
        let channels_count = buffer.channels_count;
        let callback_loop_start = loop_start_frame.unwrap_or(0);
        let callback_loop_end = loop_end_frame
            .unwrap_or(total_frames)
            .min(total_frames)
            .max(callback_loop_start + 1);

        let params = OutputDeviceParameters {
            sample_rate: buffer.sample_rate.round() as usize,
            channels_count,
            channel_sample_count: (buffer.sample_rate / 20.0).round().max(128.0) as usize,
        };

        let device = run_output_device(params, move |data| {
            for frame in data.chunks_mut(channels_count) {
                let mut frame_idx = callback_cursor.fetch_add(1, Ordering::Relaxed);
                if loop_region_ns.is_some() && frame_idx >= callback_loop_end {
                    callback_cursor.store(callback_loop_start + 1, Ordering::Relaxed);
                    frame_idx = callback_loop_start;
                }
                let sample_idx = frame_idx * channels_count;
                if sample_idx + channels_count <= callback_samples.len() {
                    frame.copy_from_slice(
                        &callback_samples[sample_idx..sample_idx + channels_count],
                    );
                } else {
                    frame.fill(0.0);
                }
            }
        })
        .map_err(|err| err.to_string())?;

        Ok(Self {
            _device: device,
            cursor_frame,
            start_time_ns: buffer.start_time_ns,
            sample_rate: buffer.sample_rate,
            total_frames,
            loop_start_frame,
        })
    }

    pub fn current_time(&self) -> TimeReal {
        let frame = self
            .cursor_frame
            .load(Ordering::Relaxed)
            .min(self.total_frames);
        TimeReal::from(self.start_time_ns + frame as f64 / self.sample_rate * 1_000_000_000.0)
    }

    pub fn is_finished(&self) -> bool {
        self.loop_start_frame.is_none()
            && self.cursor_frame.load(Ordering::Relaxed) >= self.total_frames
    }
}

struct PlaybackBuffer {
    samples: Vec<f32>,
    start_time_ns: f64,
    sample_rate: f64,
    channels_count: usize,
}

impl PlaybackBuffer {
    fn total_frames(&self) -> usize {
        self.samples.len() / self.channels_count
    }

    fn frame_for_time(&self, time: TimeInt) -> usize {
        if time == TimeInt::STATIC {
            return 0;
        }

        self.frame_for_time_ns(time.as_f64())
    }

    fn frame_for_time_ns(&self, time_ns: f64) -> usize {
        (((time_ns - self.start_time_ns).max(0.0) / 1_000_000_000.0) * self.sample_rate).floor()
            as usize
    }

    fn from_waveform(
        waveform: &AudioWaveform,
        enabled_channels: &[usize],
        mixdown: bool,
        processing: &AudioProcessingSettings,
    ) -> Result<Self, String> {
        let Some(first_chunk) = waveform.chunks.first() else {
            return Err("no audio chunks to play".to_owned());
        };

        let sample_rate = first_chunk.sample_rate;
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return Err("audio sample rate must be positive".to_owned());
        }

        let channels_count = if mixdown { 1 } else { enabled_channels.len() };
        if channels_count == 0 {
            return Err("enable at least one audio channel".to_owned());
        }

        let (start_time_ns, end_time_ns) = waveform
            .time_range_ns()
            .ok_or_else(|| "audio waveform has no finite time range".to_owned())?;
        let total_frames = (((end_time_ns - start_time_ns).max(0.0) / 1_000_000_000.0)
            * sample_rate)
            .ceil() as usize
            + 1;
        let mut samples = vec![0.0_f32; total_frames * channels_count];

        for chunk in &waveform.chunks {
            if (chunk.sample_rate - sample_rate).abs() > f64::EPSILON {
                return Err("playback requires all chunks to have the same sample rate".to_owned());
            }

            let dst_start = (((chunk.start_time.as_f64() - start_time_ns) / 1_000_000_000.0)
                * sample_rate)
                .round()
                .max(0.0) as usize;
            let chunk_frames = chunk
                .channels
                .iter()
                .map(Vec::len)
                .min()
                .unwrap_or_default();
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
                let dst_frame = dst_start + frame_idx;
                if dst_frame >= total_frames {
                    break;
                }

                if mixdown {
                    let sample = processed_mix
                        .as_ref()
                        .and_then(|mix| mix.get(frame_idx))
                        .copied()
                        .unwrap_or_default()
                        .clamp(-1.0, 1.0) as f32;
                    samples[dst_frame] = sample;
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

        Ok(Self {
            samples,
            start_time_ns,
            sample_rate,
            channels_count,
        })
    }
}
