"""Use a blueprint to show an audio view."""

import numpy as np

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_audio_view", spawn=True)

sample_rate = 16_000
seconds = 2.0
t = np.arange(int(sample_rate * seconds), dtype=np.float32) / sample_rate

samples = np.stack(
    [
        0.4 * np.sin(2.0 * np.pi * 220.0 * t),
        0.4 * np.sin(2.0 * np.pi * 330.0 * t),
    ],
    axis=1,
).astype(np.float32)

rr.set_time("time", duration=0.0)
rr.log(
    "audio",
    rr.AudioClip(
        samples,
        sample_rate=sample_rate,
        channel_names=["left", "right"],
    ),
)

blueprint = rrb.Blueprint(
    rrb.AudioView(origin="audio", name="Audio"),
    collapse_panels=True,
)
rr.send_blueprint(blueprint)
