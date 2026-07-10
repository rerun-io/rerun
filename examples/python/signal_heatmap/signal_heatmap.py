from __future__ import annotations

import argparse
import shutil
import subprocess
import wave
from pathlib import Path
from typing import cast

import numpy as np

import rerun as rr
import rerun.blueprint as rrb


def frame_signal(
    waveform: np.ndarray,
    *,
    window_size: int,
    hop_size: int,
) -> np.ndarray:
    frame_count = 1 + max(0, (waveform.shape[-1] - window_size) // hop_size)
    return np.stack(
        [waveform[..., i * hop_size : i * hop_size + window_size] for i in range(frame_count)],
        axis=-2,
    )


def stft_magnitude_db(
    waveform: np.ndarray,
    *,
    window_size: int = 1024,
    hop_size: int = 256,
    window: str = "hanning",
) -> np.ndarray:
    frames = frame_signal(waveform, window_size=window_size, hop_size=hop_size)
    window_values = window_function(window, window_size)
    spectrum = np.fft.rfft(frames * window_values, axis=-1)
    magnitude = np.abs(spectrum).astype(np.float32)
    return cast("np.ndarray", (20.0 * np.log10(np.maximum(magnitude, 1e-6))).astype(np.float32))


def window_function(name: str, window_size: int) -> np.ndarray:
    if name == "hanning":
        return np.hanning(window_size).astype(np.float32)
    if name == "hamming":
        return np.hamming(window_size).astype(np.float32)
    raise ValueError(f"unsupported window: {name}")


def fft_filter(
    waveform: np.ndarray,
    *,
    sample_rate: int,
    low_hz: float | None = None,
    high_hz: float | None = None,
) -> np.ndarray:
    frequencies = np.fft.rfftfreq(waveform.shape[-1], 1.0 / sample_rate)
    spectrum = np.fft.rfft(waveform, axis=-1)
    mask = np.ones_like(frequencies, dtype=bool)
    if low_hz is not None:
        mask &= frequencies >= low_hz
    if high_hz is not None:
        mask &= frequencies <= high_hz
    return np.fft.irfft(spectrum * mask, n=waveform.shape[-1], axis=-1).astype(np.float32)


def hz_to_mel(frequency_hz: np.ndarray) -> np.ndarray:
    return 2595.0 * np.log10(1.0 + frequency_hz / 700.0)


def mel_to_hz(mels: np.ndarray) -> np.ndarray:
    return 700.0 * (10.0 ** (mels / 2595.0) - 1.0)


def mel_filterbank(
    *,
    sample_rate: int,
    fft_size: int,
    num_mels: int = 64,
    min_hz: float = 40.0,
    max_hz: float | None = None,
) -> np.ndarray:
    max_hz = float(sample_rate / 2 if max_hz is None else max_hz)
    fft_frequencies = np.fft.rfftfreq(fft_size, 1.0 / sample_rate)
    mel_points = np.linspace(hz_to_mel(np.array([min_hz]))[0], hz_to_mel(np.array([max_hz]))[0], num_mels + 2)
    hz_points = mel_to_hz(mel_points)

    filters = np.zeros((num_mels, len(fft_frequencies)), dtype=np.float32)
    for mel_idx in range(num_mels):
        lower, center, upper = hz_points[mel_idx : mel_idx + 3]
        up_slope = (fft_frequencies - lower) / max(center - lower, 1e-6)
        down_slope = (upper - fft_frequencies) / max(upper - center, 1e-6)
        filters[mel_idx] = np.maximum(0.0, np.minimum(up_slope, down_slope))
    return filters


def mel_spectrogram_db(
    waveform: np.ndarray,
    *,
    sample_rate: int,
    window_size: int = 1024,
    hop_size: int = 256,
    num_mels: int = 64,
) -> np.ndarray:
    frames = frame_signal(waveform, window_size=window_size, hop_size=hop_size)
    window = np.hanning(window_size).astype(np.float32)
    power = np.abs(np.fft.rfft(frames * window, axis=-1)).astype(np.float32) ** 2
    mel_power = power @ mel_filterbank(sample_rate=sample_rate, fft_size=window_size, num_mels=num_mels).T
    return cast("np.ndarray", (10.0 * np.log10(np.maximum(mel_power, 1e-10))).astype(np.float32))


def mfcc(
    waveform: np.ndarray,
    *,
    sample_rate: int,
    num_mfcc: int = 20,
    num_mels: int = 64,
    window_size: int = 1024,
    hop_size: int = 256,
) -> np.ndarray:
    mel_db = mel_spectrogram_db(
        waveform,
        sample_rate=sample_rate,
        window_size=window_size,
        hop_size=hop_size,
        num_mels=num_mels,
    )
    mel_indices = np.arange(num_mels, dtype=np.float32) + 0.5
    coeff_indices = np.arange(num_mfcc, dtype=np.float32)[:, None]
    dct = np.cos(np.pi / num_mels * coeff_indices * mel_indices).astype(np.float32)
    return mel_db @ dct.T


def make_audio(sample_rate: int, duration_seconds: float) -> tuple[np.ndarray, np.ndarray]:
    t = np.arange(int(sample_rate * duration_seconds), dtype=np.float32) / sample_rate
    chirp = 0.35 * np.sin(2.0 * np.pi * (180.0 + 1400.0 * t / duration_seconds) * t)
    tone = 0.20 * np.sin(2.0 * np.pi * 880.0 * t)
    pulse = 0.65 * np.exp(-(((t - 1.3) / 0.035) ** 2))
    left = (chirp + tone + pulse).astype(np.float32)
    right = (0.7 * chirp + 0.3 * np.sin(2.0 * np.pi * 440.0 * t) + 0.45 * np.exp(-(((t - 2.1) / 0.055) ** 2))).astype(
        np.float32
    )
    return t, np.stack([left, right], axis=0)


def load_wav(path: Path) -> tuple[int, np.ndarray]:
    with wave.open(str(path), "rb") as wav:
        sample_rate = wav.getframerate()
        channels = wav.getnchannels()
        sample_width = wav.getsampwidth()
        frame_count = wav.getnframes()
        raw = wav.readframes(frame_count)

    if sample_width == 1:
        samples = (np.frombuffer(raw, dtype=np.uint8).astype(np.float32) - 128.0) / 128.0
    elif sample_width == 2:
        samples = np.frombuffer(raw, dtype="<i2").astype(np.float32) / np.iinfo(np.int16).max
    elif sample_width == 4:
        samples = np.frombuffer(raw, dtype="<i4").astype(np.float32) / np.iinfo(np.int32).max
    else:
        raise ValueError(f"unsupported WAV sample width: {sample_width} bytes")

    samples = samples.reshape(-1, channels).T
    return sample_rate, samples.astype(np.float32)


def load_audio_with_ffmpeg(path: Path, *, sample_rate: int, channels: int) -> tuple[int, np.ndarray]:
    ffmpeg = shutil.which("ffmpeg")
    if ffmpeg is None:
        raise RuntimeError("loading compressed audio requires ffmpeg on PATH")

    result = subprocess.run(
        [
            ffmpeg,
            "-v",
            "error",
            "-i",
            str(path),
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
            "-ac",
            str(channels),
            "-ar",
            str(sample_rate),
            "pipe:1",
        ],
        check=True,
        stdout=subprocess.PIPE,
    )
    samples = np.frombuffer(result.stdout, dtype="<f4").reshape(-1, channels).T
    return sample_rate, samples.astype(np.float32)


def signal_view(
    origin: str,
    name: str,
    *,
    height_dimension: int,
    indexed_dimension: int | None = None,
) -> rrb.SignalHeatmapView:
    indices = []
    slider = None
    if indexed_dimension is not None:
        indices = [rr.TensorDimensionIndexSelection(dimension=indexed_dimension, index=0)]
        slider = [indexed_dimension]

    return rrb.SignalHeatmapView(
        origin=origin,
        name=name,
        slice_selection=rrb.TensorSliceSelection(
            width=rr.TensorDimensionSelection(dimension=0),
            height=rr.TensorDimensionSelection(dimension=height_dimension, invert=True),
            indices=indices,
            slider=slider,
        ),
        scalar_mapping=rrb.TensorScalarMapping(colormap="magma"),
        view_fit="fill",
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Log precomputed signal heatmap tensors.")
    audio_input = parser.add_mutually_exclusive_group()
    audio_input.add_argument(
        "--wav",
        type=Path,
        help="Optional PCM WAV file to load instead of generating synthetic audio.",
    )
    audio_input.add_argument(
        "--audio",
        type=Path,
        help="Optional audio file to decode with ffmpeg, e.g. MP3, Opus, FLAC, or AAC.",
    )
    parser.add_argument(
        "--audio-sample-rate",
        type=int,
        default=16_000,
        help="Sample rate used when decoding --audio with ffmpeg.",
    )
    parser.add_argument(
        "--audio-channels",
        type=int,
        choices=[1, 2],
        default=2,
        help="Channel count used when decoding --audio with ffmpeg.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_signal_heatmap")

    window_size = 1024
    hop_size = 256
    if args.wav is not None:
        sample_rate, stereo = load_wav(args.wav)
    elif args.audio is not None:
        sample_rate, stereo = load_audio_with_ffmpeg(
            args.audio,
            sample_rate=args.audio_sample_rate,
            channels=args.audio_channels,
        )
    else:
        sample_rate = 16_000
        duration_seconds = 3.0
        _t, stereo = make_audio(sample_rate, duration_seconds)

    if stereo.shape[0] == 1:
        stereo = np.repeat(stereo, 2, axis=0)

    mono = stereo.mean(axis=0)

    stft = stft_magnitude_db(mono, window_size=window_size, hop_size=hop_size)
    stft_hamming = stft_magnitude_db(mono, window_size=window_size, hop_size=hop_size, window="hamming")
    low_pass = stft_magnitude_db(
        fft_filter(mono, sample_rate=sample_rate, high_hz=600.0),
        window_size=window_size,
        hop_size=hop_size,
    )
    high_pass = stft_magnitude_db(
        fft_filter(mono, sample_rate=sample_rate, low_hz=600.0),
        window_size=window_size,
        hop_size=hop_size,
    )
    band_pass = stft_magnitude_db(
        fft_filter(mono, sample_rate=sample_rate, low_hz=300.0, high_hz=1200.0),
        window_size=window_size,
        hop_size=hop_size,
    )
    mel = mel_spectrogram_db(mono, sample_rate=sample_rate, window_size=window_size, hop_size=hop_size)
    cepstra = mfcc(mono, sample_rate=sample_rate, window_size=window_size, hop_size=hop_size)
    multichannel = np.moveaxis(
        stft_magnitude_db(stereo, window_size=window_size, hop_size=hop_size),
        0,
        1,
    )

    rr.set_time("time", duration=0.0)
    rr.log(
        "audio/clip",
        rr.AudioClip(
            stereo.T,
            sample_rate=sample_rate,
            channel_names=["left", "right"],
        ),
    )

    rr.log("audio/waveform", rr.SeriesLines(names=["left", "right"]), static=True)
    for sample_idx, values in enumerate(stereo[:, ::160].T):
        rr.set_time("time", duration=float(sample_idx) * 160.0 / sample_rate)
        rr.log("audio/waveform", rr.Scalars(values))

    rr.log("features/stft", rr.Tensor(stft, dim_names=("time", "frequency")))
    rr.log("features/stft_hamming", rr.Tensor(stft_hamming, dim_names=("time", "frequency")))
    rr.log("features/low_pass", rr.Tensor(low_pass, dim_names=("time", "frequency")))
    rr.log("features/high_pass", rr.Tensor(high_pass, dim_names=("time", "frequency")))
    rr.log("features/band_pass", rr.Tensor(band_pass, dim_names=("time", "frequency")))
    rr.log("features/mel", rr.Tensor(mel, dim_names=("time", "mel")))
    rr.log("features/mfcc", rr.Tensor(cepstra, dim_names=("time", "coefficient")))
    rr.log(
        "features/multichannel_stft",
        rr.Tensor(multichannel, dim_names=("time", "channel", "frequency")),
    )

    window_frames = 24
    for offset in range(0, max(1, stft.shape[0] - window_frames), window_frames):
        rr.set_time("time", duration=float(offset * hop_size) / sample_rate)
        rr.log(
            "features/streaming_window",
            rr.Tensor(stft[offset : offset + window_frames], dim_names=("time", "frequency")),
        )

    rr.set_time("time", duration=0.0)
    for start, end, token in [
        (0.20, 0.62, "rising"),
        (0.72, 1.04, "tone"),
        (1.22, 1.42, "pulse"),
        (2.02, 2.32, "right-channel event"),
    ]:
        rr.log("asr/spans", rr.AudioAnnotation(token, span=[start, end]))
        rr.set_time("time", duration=start)
        rr.log("asr/tokens", rr.TextLog(token))

    rr.send_blueprint(
        rrb.Grid(
            rrb.AudioView(origin="/", name="Audio clip + ASR spans"),
            signal_view("features/stft", "STFT", height_dimension=1),
            signal_view("features/stft_hamming", "Hamming window STFT", height_dimension=1),
            signal_view("features/mel", "Mel", height_dimension=1),
            signal_view("features/mfcc", "MFCC", height_dimension=1),
            signal_view("features/multichannel_stft", "Channel STFT", height_dimension=2, indexed_dimension=1),
            signal_view("features/low_pass", "Low-pass STFT", height_dimension=1),
            signal_view("features/high_pass", "High-pass STFT", height_dimension=1),
            signal_view("features/band_pass", "Band-pass STFT", height_dimension=1),
            signal_view("features/streaming_window", "Streaming window", height_dimension=1),
            rrb.TimeSeriesView(origin="audio/waveform", name="Waveform"),
            rrb.TextLogView(origin="asr/tokens", name="ASR tokens"),
            grid_columns=2,
        )
    )

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
