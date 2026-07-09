# Use a blueprint to create a SignalHeatmapView.

import numpy as np

import rerun as rr
import rerun.blueprint as rrb


def stft_magnitude_db(
    waveform: np.ndarray,
    *,
    sample_rate: int,
    window_size: int = 1024,
    hop_size: int = 256,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    # Compute a basic STFT magnitude spectrogram using only NumPy.
    window = np.hanning(window_size).astype(np.float32)
    frame_count = 1 + max(0, (len(waveform) - window_size) // hop_size)
    frames = np.stack([
        waveform[i * hop_size : i * hop_size + window_size] * window
        for i in range(frame_count)
    ])
    magnitude = np.abs(np.fft.rfft(frames, axis=1)).astype(np.float32)
    spectrogram = 20.0 * np.log10(np.maximum(magnitude, 1e-6))
    times = np.arange(frame_count, dtype=np.float32) * hop_size / sample_rate
    frequencies = np.fft.rfftfreq(window_size, 1.0 / sample_rate).astype(
        np.float32
    )
    return spectrogram, times, frequencies


rr.init("rerun_example_signal_heatmap", spawn=True)

sample_rate = 16_000
duration_seconds = 2.0
t = (
    np.arange(int(sample_rate * duration_seconds), dtype=np.float32)
    / sample_rate
)

# A tiny synthetic audio signal: a rising tone plus a short pulse.
waveform = (
    0.35 * np.sin(2.0 * np.pi * (240.0 + 900.0 * t / duration_seconds) * t)
    + 0.20 * np.sin(2.0 * np.pi * 880.0 * t)
    + 0.60 * np.exp(-(((t - 1.15) / 0.035) ** 2))
).astype(np.float32)

spectrogram, times, frequencies = stft_magnitude_db(
    waveform,
    sample_rate=sample_rate,
)

rr.log("audio/waveform", rr.SeriesLines(names="waveform"), static=True)
for sample_idx, value in enumerate(waveform[::160]):
    rr.set_time("time", duration=float(sample_idx) * 160.0 / sample_rate)
    rr.log("audio/waveform", rr.Scalars(float(value)))

rr.log(
    "audio/spectrogram",
    rr.Tensor(spectrogram, dim_names=("time", "frequency")),
)

blueprint = rrb.Blueprint(
    rrb.Vertical(
        contents=[
            rrb.SignalHeatmapView(
                origin="audio/spectrogram",
                name="Spectrogram",
                slice_selection=rrb.TensorSliceSelection(
                    width=rr.TensorDimensionSelection(dimension=0),
                    height=rr.TensorDimensionSelection(
                        dimension=1, invert=True
                    ),
                ),
                scalar_mapping=rrb.TensorScalarMapping(colormap="magma"),
                view_fit="fill",
            ),
            rrb.TimeSeriesView(origin="audio/waveform", name="Waveform"),
        ]
    ),
    collapse_panels=True,
)
rr.send_blueprint(blueprint)
