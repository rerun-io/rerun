"""Tests for multi-store RRD support in RrdReader."""

from __future__ import annotations

import subprocess
from typing import TYPE_CHECKING

import pytest
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot
from rerun.experimental import RrdReader

if TYPE_CHECKING:
    from pathlib import Path


MULTI_APP_ID = "rerun_example_test_app"
REC_ID_1 = "recording_1"
REC_ID_2 = "recording_2"


@pytest.fixture(scope="session")
def blueprint_only_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """An RRD containing only a blueprint store — no recording stores."""
    path = tmp_path_factory.mktemp("blueprint_only") / "blueprint.rbl"
    rr.blueprint.Blueprint(auto_layout=False, auto_views=False).save(MULTI_APP_ID, path)
    return path


@pytest.fixture(scope="session")
def multi_store_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """
    Build a multi-store RRD with 2 recordings + 1 blueprint.

    Each source store is written to its own file, then combined into a single RRD
    (with one footer listing all three manifests) using `rerun rrd merge`.
    """
    tmp_dir = tmp_path_factory.mktemp("multi_store")
    rec1_path = tmp_dir / "rec1.rrd"
    rec2_path = tmp_dir / "rec2.rrd"
    bp_path = tmp_dir / "blueprint.rbl"
    out_path = tmp_dir / "multi.rrd"

    with rr.RecordingStream(MULTI_APP_ID, recording_id=REC_ID_1) as rec:
        rec.save(rec1_path)
        rec.send_columns(
            "/entity_a",
            indexes=[rr.TimeColumn("frame", sequence=[1, 2])],
            columns=rr.Points3D.columns(positions=[[1, 2, 3], [4, 5, 6]]),
        )

    with rr.RecordingStream(MULTI_APP_ID, recording_id=REC_ID_2) as rec:
        rec.save(rec2_path)
        rec.send_columns(
            "/entity_b",
            indexes=[rr.TimeColumn("frame", sequence=[10])],
            columns=rr.Points3D.columns(positions=[[7, 8, 9]]),
        )

    rr.blueprint.Blueprint(rr.blueprint.Spatial3DView(origin="/entity_a")).save(MULTI_APP_ID, bp_path)

    subprocess.run(
        ["rerun", "rrd", "merge", str(rec1_path), str(rec2_path), str(bp_path), "-o", str(out_path)],
        check=True,
        capture_output=True,
    )
    return out_path


# ---------------------------------------------------------------------------
# Store enumeration
# ---------------------------------------------------------------------------


def test_recordings(multi_store_rrd_path: Path) -> None:
    reader = RrdReader(multi_store_rrd_path)
    recs = reader.recordings()
    assert len(recs) == 2
    assert all(s.kind == "recording" for s in recs)
    assert {s.recording_id for s in recs} == {REC_ID_1, REC_ID_2}


def test_blueprints(multi_store_rrd_path: Path) -> None:
    reader = RrdReader(multi_store_rrd_path)
    bps = reader.blueprints()
    assert len(bps) == 1
    assert bps[0].kind == "blueprint"


def test_single_store_rrd(test_rrd_path: Path) -> None:
    """The existing single-store fixture should have exactly one recording."""
    reader = RrdReader(test_rrd_path)
    recs = reader.recordings()
    assert len(recs) == 1
    assert reader.blueprints() == []


# ---------------------------------------------------------------------------
# StoreEntry properties
# ---------------------------------------------------------------------------


def test_store_entry_properties(multi_store_rrd_path: Path) -> None:
    reader = RrdReader(multi_store_rrd_path)
    entry = reader.recordings()[0]
    assert entry.application_id == MULTI_APP_ID
    assert entry.recording_id in {REC_ID_1, REC_ID_2}
    assert entry.kind == "recording"


def test_store_entry_equality(multi_store_rrd_path: Path) -> None:
    reader = RrdReader(multi_store_rrd_path)
    entries_a = reader.recordings() + reader.blueprints()
    entries_b = reader.recordings() + reader.blueprints()
    for a, b in zip(entries_a, entries_b, strict=True):
        assert a == b


def test_store_entry_hashable(multi_store_rrd_path: Path) -> None:
    reader = RrdReader(multi_store_rrd_path)
    entries = reader.recordings() + reader.blueprints()
    entry_set = set(entries)
    assert len(entry_set) == len(entries)


def test_store_entry_repr(multi_store_rrd_path: Path) -> None:
    reader = RrdReader(multi_store_rrd_path)
    entry = reader.recordings()[0]
    assert repr(entry) == inline_snapshot(
        "StoreEntry(kind='recording', application_id='rerun_example_test_app', recording_id='recording_1')"
    )


# ---------------------------------------------------------------------------
# Store selection on stream() / store()
# ---------------------------------------------------------------------------


def test_stream_default(multi_store_rrd_path: Path) -> None:
    """Default stream() uses first recording store."""
    reader = RrdReader(multi_store_rrd_path)
    with pytest.warns(match="implicitly using"):
        chunks = list(reader.stream())
    assert len(chunks) > 0


def test_stream_specific_recording(multi_store_rrd_path: Path) -> None:
    """Can stream a specific recording by passing its StoreEntry."""
    reader = RrdReader(multi_store_rrd_path)
    recs = reader.recordings()
    assert len(recs) == 2

    chunks_0 = list(reader.stream(store=recs[0]))
    chunks_1 = list(reader.stream(store=recs[1]))
    assert len(chunks_0) > 0
    assert len(chunks_1) > 0


def test_stream_blueprint(multi_store_rrd_path: Path) -> None:
    """Streaming a blueprint store yields its chunks."""
    reader = RrdReader(multi_store_rrd_path)
    bps = reader.blueprints()
    chunks = list(reader.stream(store=bps[0]))
    assert len(chunks) > 0


def test_store_default(multi_store_rrd_path: Path) -> None:
    """Default store() loads first recording."""
    reader = RrdReader(multi_store_rrd_path)
    with pytest.warns(match="implicitly using"):
        cs = reader.store()
    assert len(cs) > 0


def test_store_specific(multi_store_rrd_path: Path) -> None:
    """Can load a specific recording as a LazyStore."""
    reader = RrdReader(multi_store_rrd_path)
    recs = reader.recordings()
    cs = reader.store(store=recs[1])
    assert len(cs) > 0


def test_implicit_pick_warns_for_stream(multi_store_rrd_path: Path) -> None:
    """stream() without `store=` should warn when there are multiple recordings."""
    reader = RrdReader(multi_store_rrd_path)
    with pytest.warns(match="implicitly using"):
        list(reader.stream())


def test_implicit_pick_silent_for_single_recording(test_rrd_path: Path) -> None:
    """No warning when the file has exactly one recording — the pick is unambiguous."""
    import warnings

    reader = RrdReader(test_rrd_path)
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        list(reader.stream())
        reader.store()


def test_stream_nonexistent_store(multi_store_rrd_path: Path, test_rrd_path: Path) -> None:
    """Streaming with a StoreEntry that doesn't belong to this file fails fast."""
    other_reader = RrdReader(test_rrd_path)
    foreign_entry = other_reader.recordings()[0]

    reader = RrdReader(multi_store_rrd_path)
    with pytest.raises(ValueError, match="not found"):
        reader.stream(store=foreign_entry)


def test_store_nonexistent_store(multi_store_rrd_path: Path, test_rrd_path: Path) -> None:
    """Loading a store with a StoreEntry that doesn't belong to this file fails fast."""
    other_reader = RrdReader(test_rrd_path)
    foreign_entry = other_reader.recordings()[0]

    reader = RrdReader(multi_store_rrd_path)
    with pytest.raises(ValueError, match="not found"):
        reader.store(store=foreign_entry)


def test_stream_default_no_recording_raises(blueprint_only_rrd_path: Path) -> None:
    """stream() with no explicit store and no recording in the file must fail fast."""
    reader = RrdReader(blueprint_only_rrd_path)
    with pytest.raises(ValueError, match="No recording store"):
        reader.stream()


def test_store_default_no_recording_raises(blueprint_only_rrd_path: Path) -> None:
    """store() with no explicit store and no recording in the file must fail fast."""
    reader = RrdReader(blueprint_only_rrd_path)
    with pytest.raises(ValueError, match="No recording store"):
        reader.store()


# ---------------------------------------------------------------------------
# Backward compatibility with the single-store fixture
# ---------------------------------------------------------------------------


def test_existing_stream_unchanged(test_rrd_path: Path) -> None:
    """Existing single-store usage still works."""
    reader = RrdReader(test_rrd_path)
    chunks = list(reader.stream())
    assert len(chunks) > 0


def test_existing_store_unchanged(test_rrd_path: Path) -> None:
    """Existing single-store store() still works."""
    reader = RrdReader(test_rrd_path)
    cs = reader.store()
    assert len(cs) > 0
