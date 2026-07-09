"""Create and log a multi-channel audio clip."""

import numpy as np

import rerun as rr

rr.init("rerun_example_audio_clip", spawn=True)

sample_rate = 16_000
seconds = 2.0
t = np.arange(int(sample_rate * seconds), dtype=np.float32) / sample_rate

left = 0.4 * np.sin(2.0 * np.pi * 220.0 * t)
right = 0.4 * np.sin(2.0 * np.pi * 330.0 * t)
samples = np.stack([left, right], axis=1).astype(np.float32)

rr.set_time("time", duration=0.0)
rr.log(
    "audio",
    rr.AudioClip(
        samples,
        sample_rate=sample_rate,
        channel_names=["left", "right"],
    ),
)
