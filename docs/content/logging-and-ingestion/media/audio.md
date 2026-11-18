---
title: Audio
order: 200
description: How to log audio with AssetAudio, including raw PCM samples
---

Rerun plays back audio files logged with the [`AssetAudio`](../../reference/types/archetypes/asset_audio.md) archetype.
See its reference page for the supported formats, how playback lines up with time, and an example.

## Raw PCM samples

There is [no archetype for raw PCM samples yet](../../development/roadmap.md#missing-features).
Instead, encode the samples as a WAV file in memory and log that.
For live audio, do this for short segments, and log each segment at the time it starts:

```python
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
samples = (np.sin(2 * np.pi * 440 * t) * 10_000).astype(np.int16)  # One second of 440 Hz

rr.set_time("time", duration=0.0)
rr.log("audio", rr.AssetAudio(contents=wav_bytes(samples, sample_rate), media_type="audio/wav"))
```

To plot the raw signal instead of playing it, log the samples as [`Scalars`](../../reference/types/archetypes/scalars.md), for example with [`send_columns`](../send-data/send-columns.md).
