"""
Tests for rr.send_dataframe and rr.send_record_batch.

These tests verify the send_dataframe functionality using the Server + Catalog API.
"""

from __future__ import annotations

import uuid
from typing import TYPE_CHECKING

import rerun as rr

if TYPE_CHECKING:
    from pathlib import Path

    import pyarrow as pa
    from syrupy import SnapshotAssertion

APP_ID = "rerun_example_test_send_dataframe"


def _filter_rerun_columns(table: pa.Table) -> pa.Table:
    """Filter to only include columns with proper rerun metadata (skip rerun_segment_id)."""

    cols_to_keep = []
    for field in table.schema:
        if field.name == "log_time":
            # changes every run
            continue

        if field.metadata is None or b"rerun:kind" not in field.metadata:
            continue

        cols_to_keep.append(field.name)
    return table.select(cols_to_keep)


def test_send_dataframe_roundtrip(tmp_path: Path, snapshot: SnapshotAssertion) -> None:
    """Test that send_dataframe can roundtrip data through Server + Catalog API."""
    original_dir = tmp_path / "original"
    original_dir.mkdir()
    rrd_path = original_dir / "recording.rrd"

    # Create initial recording with some data
    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(str(rrd_path))
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3], [4, 5, 6], [7, 8, 9]], radii=[0.5]))
        rec.set_time("my_index", sequence=7)
        rec.log("points", rr.Points3D([[10, 11, 12]], colors=[[255, 0, 0]]))

    # Load via Server + Catalog API and read as Arrow table
    with rr.server.Server(datasets={"test_dataset": original_dir}) as server:
        ds = server.client().get_dataset("test_dataset")
        original_table = _filter_rerun_columns(ds.reader(index="my_index").to_arrow_table())

    # Send via send_dataframe to a new recording
    roundtrip_dir = tmp_path / "roundtrip"
    roundtrip_dir.mkdir()
    rrd2_path = roundtrip_dir / "recording.rrd"
    with rr.RecordingStream(APP_ID + "_roundtrip", recording_id=uuid.uuid4()) as rec2:
        rec2.save(str(rrd2_path))
        rr.send_dataframe(original_table, recording=rec2)

    # Verify roundtrip via catalog API - data should be identical
    with rr.server.Server(datasets={"roundtrip_dataset": roundtrip_dir}) as server:
        ds = server.client().get_dataset("roundtrip_dataset")
        roundtrip_table = _filter_rerun_columns(ds.reader(index="my_index").to_arrow_table())

    assert original_table == roundtrip_table
    assert str(original_table) == snapshot()
