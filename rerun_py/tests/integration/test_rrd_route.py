from __future__ import annotations

import subprocess
from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.chunk import RrdReader

if TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.parametrize(
    "timelines",
    [
        pytest.param([None, "step"], id="static-then-temporal"),
        pytest.param(["step", None], id="temporal-then-static"),
        pytest.param(["step", "frame"], id="different-timelines"),
        pytest.param(["frame", "step"], id="different-timelines-reversed"),
        pytest.param([None, None], id="matching-static-schemas"),
        pytest.param(["step", "step"], id="matching-temporal-schemas"),
        pytest.param([None, "step", "frame"], id="three-inputs"),
    ],
)
def test_route_different_manifest_schemas(tmp_path: Path, timelines: list[str | None]) -> None:
    inputs = []

    for i, timeline in enumerate(timelines):
        path = tmp_path / f"input-{i}.rrd"
        with rr.RecordingStream("rerun_example_route_test", recording_id=f"recording-{i}") as rec:
            rec.save(path)
            if timeline is not None:
                rec.set_time(timeline, sequence=10 + i)
            rec.log(
                f"entity_{i}",
                rr.Points3D([[float(i), 0.0, 0.0]]),
                static=timeline is None,
            )
        inputs.append(path)

    output = tmp_path / "combined.rrd"

    def run_rrd(*args: str) -> None:
        result = subprocess.run(
            ["rerun", "rrd", *args],
            capture_output=True,
            text=True,
            check=False,
        )
        assert result.returncode == 0, result.stdout + result.stderr

    run_rrd(
        "route",
        "--recording-id",
        "combined",
        *(str(path) for path in inputs),
        "-o",
        str(output),
    )
    run_rrd("verify", str(output))

    reader = RrdReader(output)
    recordings = reader.recordings()
    assert len(recordings) == 1
    assert recordings[0].recording_id == "combined"

    store = reader.stream().collect()
    assert {f"/entity_{i}" for i in range(len(inputs))}.issubset(set(store.schema().entity_paths()))
