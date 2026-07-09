"""Create and log audio annotation spans."""

import rerun as rr

rr.init("rerun_example_audio_annotation", spawn=True)

rr.set_time("time", duration=0.0)
rr.log("audio/asr", rr.AudioAnnotation("hello", span=[0.00, 0.32]))
rr.log("audio/asr", rr.AudioAnnotation("world", span=[0.34, 0.72]))
