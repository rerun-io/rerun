"""Tests for rerun.experimental.McapLoader and StreamingLoader protocol."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from rerun.experimental import Chunk, McapLoader, StreamingLoader

if TYPE_CHECKING:
    from syrupy import SnapshotAssertion

MCAP_ASSETS_DIR = (
    Path(__file__).resolve().parents[3]
    / "crates"
    / "store"
    / "re_data_loader"
    / "src"
    / "loader_mcap"
    / "tests"
    / "assets"
)

POINT_CLOUD_MCAP = MCAP_ASSETS_DIR / "foxglove_point_cloud.mcap"
LOG_MCAP = MCAP_ASSETS_DIR / "foxglove_log.mcap"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def chunk_summary(chunks: list[Chunk]) -> str:
    """
    Compact, deterministic summary of a chunk list for snapshot testing.

    Includes entity path, row count, static flag, timeline names, and
    component column names — enough to detect regressions in decoding
    without being sensitive to formatting changes.
    """
    lines = []
    for c in sorted(chunks, key=lambda c: (c.entity_path, not c.is_static)):
        rb = c.to_record_batch()
        cols = sorted(f.name for f in rb.schema if not f.name.startswith("rerun.controls"))
        timelines = sorted(c.timeline_names)
        lines.append(f"{c.entity_path}  rows={c.num_rows}  static={c.is_static}  timelines={timelines}  cols={cols}")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Core: default load produces expected chunks
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_load_point_cloud(snapshot: SnapshotAssertion) -> None:
    """Default load of point cloud MCAP: correct entities, components, and timelines."""
    chunks = McapLoader(POINT_CLOUD_MCAP).stream().collect()
    assert chunk_summary(chunks) == snapshot


@pytest.mark.local_only
def test_load_log(snapshot: SnapshotAssertion) -> None:
    """Default load of log MCAP: TextLog with 6 rows."""
    chunks = McapLoader(LOG_MCAP).stream().collect()
    assert chunk_summary(chunks) == snapshot


# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_file_not_found(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError, match="not found"):
        McapLoader(tmp_path / "nonexistent.mcap")


@pytest.mark.local_only
def test_invalid_timeline_type() -> None:
    with pytest.raises(ValueError, match="Invalid timeline_type"):
        McapLoader(POINT_CLOUD_MCAP, timeline_type="sequence")  # type: ignore[arg-type]


# ---------------------------------------------------------------------------
# Loader parameters
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_decoders_protobuf_only(snapshot: SnapshotAssertion) -> None:
    """Selecting only protobuf decoder still produces data (point cloud is protobuf-encoded)."""
    chunks = McapLoader(POINT_CLOUD_MCAP, decoders=["protobuf"]).stream().collect()
    assert chunk_summary(chunks) == snapshot


@pytest.mark.local_only
def test_decoders_empty() -> None:
    """Empty decoder list produces no chunks at all."""
    chunks = McapLoader(POINT_CLOUD_MCAP, decoders=[]).stream().collect()
    assert len(chunks) == 0


@pytest.mark.local_only
def test_timeline_type_duration() -> None:
    """Duration timeline type changes Arrow field types from timestamp[ns] to duration[ns]."""
    chunks = McapLoader(LOG_MCAP, timeline_type="duration").stream().collect()
    temporal = [c for c in chunks if not c.is_static]
    rb = temporal[0].to_record_batch()
    ts_field = next(f for f in rb.schema if f.name == "timestamp")
    assert ts_field.type == pa.duration("ns")


@pytest.mark.local_only
def test_timestamp_offset() -> None:
    """Offset shifts all timestamp timelines by the given amount."""
    offset_ns = 1_000_000_000

    def first_timestamp_ns(chunks: list[Chunk]) -> int:
        for c in sorted(chunks, key=lambda c: c.entity_path):
            if not c.is_static:
                rb = c.to_record_batch()
                ts_col = rb.column("timestamp")
                return int(ts_col[0].as_py().value)
        raise AssertionError("no temporal chunk found")

    base_ts = first_timestamp_ns(McapLoader(LOG_MCAP).stream().collect())
    offset_ts = first_timestamp_ns(McapLoader(LOG_MCAP, timestamp_offset_ns=offset_ns).stream().collect())

    assert offset_ts - base_ts == offset_ns


# ---------------------------------------------------------------------------
# StreamingLoader protocol conformance
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_streaming_loader_protocol() -> None:
    assert isinstance(McapLoader(POINT_CLOUD_MCAP), StreamingLoader)
