"""Use a blueprint to show and play an audio asset."""

import sys

import rerun as rr
import rerun.blueprint as rrb

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_audio.[aac|flac|m4a|mp3|ogg|wav]>")
    sys.exit(1)

rr.init("rerun_example_audio_view", spawn=True)

rr.set_time("time", duration=0.0)
rr.log("audio", rr.AssetAudio(path=sys.argv[1]))

blueprint = rrb.Blueprint(
    rrb.AudioView(
        origin="audio",
        name="Audio",
        playback=rrb.AudioPlayback(volume=0.8),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
