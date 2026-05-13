import math

import rerun as rr

with rr.RecordingStream("rerun_example_getting_started", recording_id="run-1", send_properties=False) as rec:
    rec.save("run-1.rrd")
    for t in range(10):
        rec.set_time("t", duration=t)
        rec.log("/arm/shoulder", rr.Scalars(math.sin(t * 0.5)))
        rec.log("/arm/elbow", rr.Scalars(math.cos(t * 0.5)))
