"""
Stress test for things that tend to GIL deadlock.

Logs many large recordings that contain a lot of large rows.

Usage:
```
python main.py
"""
from __future__ import annotations

import rerun as rr

rec = rr.new_recording(application_id="test")

rec = rr.new_recording(application_id="test")
rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

rec = rr.new_recording(application_id="test", make_default=True)
rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

rec = rr.new_recording(application_id="test", make_thread_default=True)
rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

rec = rr.new_recording(application_id="test")  # this works
rr.set_global_data_recording(rec)
rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

rec = rr.new_recording(application_id="test")  # this works
rr.set_thread_local_data_recording(rec)
rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

rec = rr.new_recording(application_id="test", spawn=True)
rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

rec = rr.new_recording(application_id="test")
rr.connect(recording=rec)
rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

rec = rr.new_recording(application_id="test")
rr.memory_recording(recording=rec)
rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

for _ in range(3):
    rec = rr.new_recording(application_id="test", make_default=False, make_thread_default=False)
    mem = rec.memory_recording()
    rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)

for _ in range(3):
    rec = rr.new_recording(application_id="test", make_default=False, make_thread_default=False)
    rr.log("test", rr.Points3D([1, 2, 3]), recording=rec.inner)
