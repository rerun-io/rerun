<!--[metadata]
title = "Signal heatmap"
tags = ["Audio", "Tensor", "Plots", "API example"]
channel = "main"
include_in_manifest = true
-->

This example logs a synthetic multi-channel audio clip, precomputed signal features, and timestamped ASR-style text so they can be inspected on the same Rerun timeline.

Pass `--wav path/to/audio.wav` to load a PCM WAV file instead of using the synthetic signal.

The example intentionally computes the audio features in user code. Rerun displays the raw PCM audio clip and resulting tensors, but does not perform the spectral transforms itself. It includes small NumPy-only helpers for:

- STFT magnitude spectrograms: `[time, frequency]`
- Mel spectrograms: `[time, mel]`
- MFCCs: `[time, coefficient]`
- Multi-channel spectrograms: `[time, channel, frequency]`
- Streaming/rolling spectrogram windows: `[time, frequency]`
- Window comparison with Hanning and Hamming windows
- Simple low-pass, high-pass, and band-pass filtered spectrograms
- ASR-style word spans in `AudioView`, plus timestamped tokens in a `TextLogView`

## Used Rerun types

[`AudioClip`](https://www.rerun.io/docs/reference/types/archetypes/audio_clip), [`AudioAnnotation`](https://www.rerun.io/docs/reference/types/archetypes/audio_annotation), [`AudioView`](https://www.rerun.io/docs/reference/types/views/audio_view), [`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`SignalHeatmapView`](https://www.rerun.io/docs/reference/types/views/signal_heatmap_view), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars), [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines), [`TextLog`](https://www.rerun.io/docs/reference/types/archetypes/text_log), [`TextLogView`](https://www.rerun.io/docs/reference/types/views/text_log_view)
