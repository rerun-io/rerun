"""
Stress test for things that tend to GIL deadlock.

Logs many large recordings that contain a lot of large rows.

Usage:
```
python main.py
"""

from __future__ import annotations

import rerun as rr

rec = rr.RecordingStream(application_id="test")

rec = rr.RecordingStream(application_id="test")
rec.log("test", rr.Points3D([1, 2, 3]))

rec = rr.RecordingStream(application_id="test", make_default=True)
rec.log("test", rr.Points3D([1, 2, 3]))

rec = rr.RecordingStream(application_id="test", make_thread_default=True)
rec.log("test", rr.Points3D([1, 2, 3]))

rec = rr.RecordingStream(application_id="test")  # this works
rr.set_global_data_recording(rec)
rec.log("test", rr.Points3D([1, 2, 3]))

rec = rr.RecordingStream(application_id="test")  # this works
rr.set_thread_local_data_recording(rec)
rec.log("test", rr.Points3D([1, 2, 3]))

rec = rr.RecordingStream(application_id="test")
rec.spawn()
rec.log("test", rr.Points3D([1, 2, 3]))

rec = rr.RecordingStream(application_id="test")
rr.connect_grpc(recording=rec)
rec.log("test", rr.Points3D([1, 2, 3]))

rec = rr.RecordingStream(application_id="test")
rr.memory_recording(recording=rec)
rec.log("test", rr.Points3D([1, 2, 3]))

for _ in range(3):
    rec = rr.RecordingStream(application_id="test", make_default=False, make_thread_default=False)
    mem = rec.memory_recording()
    rec.log("test", rr.Points3D([1, 2, 3]))

for _ in range(3):
    rec = rr.RecordingStream(application_id="test", make_default=False, make_thread_default=False)
    rec.log("test", rr.Points3D([1, 2, 3]))
