"""Save a small recording to RRD and inspect a chunk from it."""

from __future__ import annotations

import atexit
import tempfile
from pathlib import Path

import rerun as rr

_tmp_dir = tempfile.TemporaryDirectory()
atexit.register(_tmp_dir.cleanup)
output_path = Path(_tmp_dir.name) / "output.rrd"

# region: write
with rr.RecordingStream(
    "rerun_example_rrd_format", recording_id="example"
) as rec:
    rec.save(output_path)
    rec.set_time("frame", sequence=0)
    rec.log(
        "/points",
        rr.Points3D(
            [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]],
            colors=[(255, 0, 0), (0, 255, 0)],
        ),
    )
    rec.set_time("frame", sequence=1)
    rec.log("/points", rr.Points3D([[2.0, 2.0, 2.0]], colors=[(0, 0, 255)]))
# endregion: write

# region: inspect
from rerun.experimental import RrdReader

reader = RrdReader(output_path)
for chunk in reader.stream():
    if chunk.entity_path == "/points":
        print(chunk.format(trim_metadata_keys=False))
        break
# endregion: inspect
