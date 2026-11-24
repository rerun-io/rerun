"""Log raw PCM samples as audio, by encoding them as a WAV file in memory."""

import io
import wave

import numpy as np

import rerun as rr


def wav_bytes(samples: np.ndarray, sample_rate: int) -> bytes:
    """Encode mono int16 samples as a WAV file."""
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)  # 16-bit
        wav.setframerate(sample_rate)
        wav.writeframes(samples.astype(np.int16).tobytes())
    return buf.getvalue()


rr.init("rerun_example_pcm_audio", spawn=True)

sample_rate = 16_000
t = np.arange(sample_rate) / sample_rate
# One second of 440 Hz
samples = (np.sin(2 * np.pi * 440 * t) * 10_000).astype(np.int16)

rr.set_time("time", duration=0.0)
rr.log(
    "audio",
    rr.AssetAudio(
        contents=wav_bytes(samples, sample_rate), media_type="audio/wav"
    ),
)
