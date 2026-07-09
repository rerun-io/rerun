<!--[metadata]
title = "Signal heatmap"
tags = ["Audio", "Tensor", "Plots", "API example"]
channel = "main"
include_in_manifest = true
-->

This example logs precomputed signal features as tensors and visualizes them with `SignalHeatmapView`.

The example intentionally computes the audio features in user code. Rerun displays the resulting tensors, but does not perform the spectral transforms itself. It includes small NumPy-only helpers for:

- STFT magnitude spectrograms: `[time, frequency]`
- Mel spectrograms: `[time, mel]`
- MFCCs: `[time, coefficient]`
- Multi-channel spectrograms: `[time, channel, frequency]`
- Streaming/rolling spectrogram windows: `[time, frequency]`

## Used Rerun types

[`Tensor`](https://www.rerun.io/docs/reference/types/archetypes/tensor), [`SignalHeatmapView`](https://www.rerun.io/docs/reference/types/views/signal_heatmap_view), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars), [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines)
