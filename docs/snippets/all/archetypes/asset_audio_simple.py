"""Log an audio file at t=0 on the `time` timeline."""

import sys

import rerun as rr

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_audio.[aac|flac|m4a|mp3|ogg|wav]>")
    sys.exit(1)

rr.init("rerun_example_asset_audio", spawn=True)

rr.set_time("time", duration=0.0)
rr.log("audio", rr.AssetAudio(path=sys.argv[1]))
