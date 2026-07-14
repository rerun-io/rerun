"""Tests for rerun.experimental.McapReader and StreamingReader protocol."""

from __future__ import annotations

import re
from pathlib import Path
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
from rerun.experimental import Chunk, McapReader, StreamingReader

if TYPE_CHECKING:
    from syrupy import SnapshotAssertion

MCAP_ASSETS_DIR = (
    Path(__file__).resolve().parents[3]
    / "crates"
    / "store"
    / "re_importer"
    / "src"
    / "importer_mcap"
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


def test_load_point_cloud(snapshot: SnapshotAssertion) -> None:
    """Default load of point cloud MCAP: correct entities, components, and timelines."""
    chunks = McapReader(POINT_CLOUD_MCAP).stream().to_chunks()
    assert chunk_summary(chunks) == snapshot


def test_load_log(snapshot: SnapshotAssertion) -> None:
    """Default load of log MCAP: TextLog with 6 rows."""
    chunks = McapReader(LOG_MCAP).stream().to_chunks()
    assert chunk_summary(chunks) == snapshot


# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------


def test_file_not_found(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError, match="not found"):
        McapReader(tmp_path / "nonexistent.mcap")


def test_invalid_timeline_type() -> None:
    with pytest.raises(ValueError, match="Invalid timeline_type"):
        McapReader(POINT_CLOUD_MCAP, timeline_type="sequence")  # type: ignore[arg-type]


# ---------------------------------------------------------------------------
# Reader parameters
# ---------------------------------------------------------------------------


def test_decoders_protobuf_only(snapshot: SnapshotAssertion) -> None:
    """Selecting only protobuf decoder still produces data (point cloud is protobuf-encoded)."""
    chunks = McapReader(POINT_CLOUD_MCAP, decoders=["protobuf"]).stream().to_chunks()
    assert chunk_summary(chunks) == snapshot


def test_decoders_empty() -> None:
    """Empty decoder list produces no chunks at all."""
    chunks = McapReader(POINT_CLOUD_MCAP, decoders=[]).stream().to_chunks()
    assert len(chunks) == 0


def test_timeline_type_duration() -> None:
    """Duration timeline type changes Arrow field types from timestamp[ns] to duration[ns]."""
    chunks = McapReader(LOG_MCAP, timeline_type="duration").stream().to_chunks()
    temporal = [c for c in chunks if not c.is_static]
    rb = temporal[0].to_record_batch()
    ts_field = next(f for f in rb.schema if f.name == "timestamp")
    assert ts_field.type == pa.duration("ns")


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

    base_ts = first_timestamp_ns(McapReader(LOG_MCAP).stream().to_chunks())
    offset_ts = first_timestamp_ns(McapReader(LOG_MCAP, timestamp_offset_ns=offset_ns).stream().to_chunks())

    assert offset_ts - base_ts == offset_ns


# ---------------------------------------------------------------------------
# Topic filter
# ---------------------------------------------------------------------------


def _entity_paths(chunks: list[Chunk]) -> set[str]:
    return {c.entity_path for c in chunks}


def test_topic_filter_include_all() -> None:
    """A regex matching everything is equivalent to no filter."""
    baseline = _entity_paths(McapReader(POINT_CLOUD_MCAP).stream().to_chunks())
    filtered = _entity_paths(McapReader(POINT_CLOUD_MCAP, include_topic_regex=[".*"]).stream().to_chunks())
    assert filtered == baseline


def test_topic_filter_include_none() -> None:
    """A regex matching no topic produces only file-scoped chunks (no per-topic chunks)."""
    baseline = _entity_paths(McapReader(POINT_CLOUD_MCAP).stream().to_chunks())
    filtered = _entity_paths(
        McapReader(POINT_CLOUD_MCAP, include_topic_regex=["^__definitely_not_a_topic__$"]).stream().to_chunks(),
    )
    # Filtered set must be a (proper) subset of baseline; any remaining entries
    # come from file-scoped decoders (schemas, statistics, recording_info, …)
    # which are independent of the topic filter.
    assert filtered.issubset(baseline)
    assert len(filtered) < len(baseline)


def test_topic_filter_include_specific() -> None:
    """Including one specific topic yields a strict subset containing that topic."""
    baseline_chunks = McapReader(POINT_CLOUD_MCAP).stream().to_chunks()
    baseline = _entity_paths(baseline_chunks)

    # Pick the entity path of any non-static chunk — those come from real topics.
    target = next(c.entity_path for c in baseline_chunks if not c.is_static)
    # The entity path is constructed from the topic verbatim, so it doubles as a
    # regex that matches exactly that topic when escaped.
    pattern = "^" + re.escape(target) + "$"

    filtered = _entity_paths(McapReader(POINT_CLOUD_MCAP, include_topic_regex=[pattern]).stream().to_chunks())

    assert target in filtered
    assert filtered.issubset(baseline)


def test_topic_filter_exclude() -> None:
    """Excluding one topic drops it from the output."""
    baseline_chunks = McapReader(POINT_CLOUD_MCAP).stream().to_chunks()
    target = next(c.entity_path for c in baseline_chunks if not c.is_static)
    pattern = "^" + re.escape(target) + "$"

    filtered = _entity_paths(McapReader(POINT_CLOUD_MCAP, exclude_topic_regex=[pattern]).stream().to_chunks())

    assert target not in filtered


def test_topic_filter_invalid_regex() -> None:
    """Bad regex syntax raises ValueError naming the offending pattern."""
    with pytest.raises(ValueError, match="include topic regex"):
        McapReader(POINT_CLOUD_MCAP, include_topic_regex=["["])
    with pytest.raises(ValueError, match="exclude topic regex"):
        McapReader(POINT_CLOUD_MCAP, exclude_topic_regex=["["])


# ---------------------------------------------------------------------------
# StreamingReader protocol conformance
# ---------------------------------------------------------------------------


def test_streaming_reader_protocol() -> None:
    assert isinstance(McapReader(POINT_CLOUD_MCAP), StreamingReader)
