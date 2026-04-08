"""Shared fixtures for integration tests."""

from __future__ import annotations

from typing import TYPE_CHECKING

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

    return rrd_path
