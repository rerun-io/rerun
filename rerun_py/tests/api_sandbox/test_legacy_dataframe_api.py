from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import rerun as rr
from inline_snapshot import snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_legacy_api(simple_recording_path: Path) -> None:
    recording = rr.dataframe.load_recording(simple_recording_path)

    assert str(recording.schema()) == snapshot("""\
Index(timeline:timeline)
Column name: /points:Points2D:colors
	Entity path: /points
	Archetype: rerun.archetypes.Points2D
	Component type: rerun.components.Color
	Component: Points2D:colors
Column name: /points:Points2D:positions
	Entity path: /points
	Archetype: rerun.archetypes.Points2D
	Component type: rerun.components.Position2D
	Component: Points2D:positions
Column name: property:RecordingInfo:start_time
	Entity path: /__properties
	Archetype: rerun.archetypes.RecordingInfo
	Component type: rerun.components.Timestamp
	Component: RecordingInfo:start_time
	Static: true\
""")

    view = recording.view(index="timeline", contents="/**")

    assert str(view.schema()) == snapshot("""\
Index(timeline:timeline)
Column name: /points:Points2D:colors
	Entity path: /points
	Archetype: rerun.archetypes.Points2D
	Component type: rerun.components.Color
	Component: Points2D:colors
Column name: /points:Points2D:positions
	Entity path: /points
	Archetype: rerun.archetypes.Points2D
	Component type: rerun.components.Position2D
	Component: Points2D:positions\
""")

    view = view.fill_latest_at()

    record_batch_reader = view.select()
    table = pa.Table.from_batches(record_batch_reader)

    assert str(table) == snapshot("""\
pyarrow.Table
timeline: timestamp[ns]
/points:Points2D:colors: list<item: uint32>
  child 0, item: uint32
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
  child 0, item: fixed_size_list<item: float not null>[2]
      child 0, item: float not null
----
timeline: [[2000-01-01 00:00:00.000000000]]
/points:Points2D:colors: [[[4278190335,16711935]]]
/points:Points2D:positions: [[[[0,1],[3,4]]]]\
""")
