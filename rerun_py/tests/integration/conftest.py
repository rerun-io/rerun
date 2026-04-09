"""Shared fixtures for integration tests."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
import rerun as rr

if TYPE_CHECKING:
    from pathlib import Path

TEST_APP_ID = "integration_tests"
TEST_RECORDING_ID = "fixed-recording-id-for-integration-tests"


@pytest.fixture(scope="session")
def test_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Session-scoped RRD with known entity paths, timelines, and component structure."""

    rrd_path = tmp_path_factory.mktemp("integration") / "test.rrd"

    with rr.RecordingStream(TEST_APP_ID, recording_id=TEST_RECORDING_ID) as rec:
        rec.save(rrd_path)

        # Temporal: two timelines, Points3D with positions + colors
        rec.send_columns(
            "/robots/arm",
            indexes=[
                rr.TimeColumn("my_index", sequence=[1, 2]),
                rr.TimeColumn("other_timeline", sequence=[10, 20]),
            ],
            columns=rr.Points3D.columns(
                positions=[[1, 2, 3], [4, 5, 6]],
                colors=[[255, 0, 0], [0, 255, 0]],
            ),
        )

        # Temporal: one timeline, TextLog
        rec.send_columns(
            "/cameras/front",
            indexes=[rr.TimeColumn("my_index", sequence=[1])],
            columns=rr.TextLog.columns(text=["frame_001"]),
        )

        # Static: no timelines, TextLog
        rec.send_columns(
            "/config",
            indexes=[],
            columns=rr.TextLog.columns(text=["v1"]),
        )

        # Temporal: dynamic archetype with a struct component (for selector field access tests)
        imu_data = pa.StructArray.from_arrays(
            [
                pa.array([0.1, 0.4], type=pa.float64()),
                pa.array([0.2, 0.5], type=pa.float64()),
                pa.array([9.8, 9.7], type=pa.float64()),
                pa.array([1000000000, 2000000000], type=pa.int64()),
            ],
            names=["x", "y", "z", "timestamp"],
        )
        rec.send_columns(
            "/sensors/imu",
            indexes=[rr.TimeColumn("my_index", sequence=[1, 2])],
            columns=rr.DynamicArchetype.columns(
                archetype="Imu",
                components={"accel": imu_data},
            ),
        )

    return rrd_path
