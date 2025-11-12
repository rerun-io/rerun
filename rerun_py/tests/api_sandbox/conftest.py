"""Common fixture used by all tests."""

from __future__ import annotations

import datetime
import sys
from pathlib import Path
from typing import TYPE_CHECKING

import pytest
import rerun as rr

if TYPE_CHECKING:
    from collections.abc import Iterator


RERUN_DRAFT_PATH = str(Path(__file__).parent)

if RERUN_DRAFT_PATH not in sys.path:
    sys.path.insert(0, RERUN_DRAFT_PATH)


def create_simple_rrd(rrd_path: Path, recording_id: str, data_start_value: int) -> None:
    with rr.RecordingStream("rerun_example_api_test", recording_id=recording_id) as rec:
        rec.save(rrd_path)

        # Avoid `rec.log()` so we dont have the default timelines
        rec.send_columns(
            "/points",
            [rr.TimeColumn("timeline", timestamp=[datetime.datetime(2000, 1, 1, 0, 0, data_start_value)])],
            [
                *rr.Points2D.columns(
                    positions=[[data_start_value, data_start_value + 1], [data_start_value + 3, data_start_value + 4]],
                    colors=[[255, 0, data_start_value % 255], [0, 255, data_start_value % 255]],
                ).partition([2])
            ],
        )


@pytest.fixture(scope="session")
def simple_recording_path(tmp_path_factory: pytest.TempPathFactory) -> Iterator[Path]:
    """Create a temporary recording with little but predicatable content."""

    rrd_path = tmp_path_factory.mktemp("simple_recording") / "simple_recording.rrd"
    create_simple_rrd(rrd_path, "simple_recording_id", 0)
    yield rrd_path


@pytest.fixture(scope="session")
def simple_dataset_prefix(tmp_path_factory: pytest.TempPathFactory) -> Iterator[Path]:
    """Create a temporary dataset prefix with a few simple recordings."""

    prefix_path = tmp_path_factory.mktemp("simple_dataset_prefix")

    for i in range(3):
        create_simple_rrd(prefix_path / f"simple_recording_{i}.rrd", f"simple_recording_{i}", i)

    yield prefix_path
