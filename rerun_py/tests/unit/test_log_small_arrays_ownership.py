"""Small arrays logged from Python must not keep pointing at the caller's memory after `rr.log` returns."""

from __future__ import annotations

import gc
import weakref
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa
import pytest
import rerun as rr
from rerun.chunk import RrdReader

if TYPE_CHECKING:
    from pathlib import Path


def _position_rows(path: Path) -> list[list[float]]:
    rows: list[list[float]] = []
    for chunk in RrdReader(path).stream().to_chunks():
        if chunk.entity_path != "/e":
            continue
        batch = chunk.to_record_batch()
        for name in batch.schema.names:
            if name.endswith(":positions"):
                for cell in batch.column(name).to_pylist():
                    rows.extend(cell)
    return rows


def _log_to_file(path: Path, archetype: Any, *, mutate_after_log: Any = None) -> None:
    rr.init("rerun_example_small_array_ownership", recording_id=path.stem)
    rr.save(path)
    rr.log("e", archetype)
    if mutate_after_log is not None:
        mutate_after_log()
    rr.disconnect()


def test_numpy_memory_is_released_when_log_returns() -> None:
    # Before the fix the row kept the imported array, and with it the numpy buffer, until the batcher flushed it.
    rr.init("rerun_example_small_array_ownership")
    rr.memory_recording()

    positions = np.array([[1, 2, 3]], dtype=np.float32)
    alive = weakref.ref(positions)

    rr.log("e", rr.Points3D(positions))
    del positions
    gc.collect()

    assert alive() is None, "the logged row still references the caller's numpy array"


def test_mutating_the_array_after_logging_does_not_change_the_logged_data(tmp_path: Path) -> None:
    positions = np.array([[1, 2, 3]], dtype=np.float32)

    def mutate() -> None:
        positions[:] = 99

    _log_to_file(tmp_path / "mutate.rrd", rr.Points3D(positions), mutate_after_log=mutate)

    assert _position_rows(tmp_path / "mutate.rrd") == [[1.0, 2.0, 3.0]]


def test_sliced_arrow_array_is_logged_correctly(tmp_path: Path) -> None:
    # An array with a non-zero offset, which a naive buffer copy would get wrong.
    batch_type = rr.components.Position3DBatch._ARROW_DATATYPE
    full = pa.FixedSizeListArray.from_arrays(pa.array(np.arange(12, dtype=np.float32)), type=batch_type)
    sliced = full.slice(1, 2)

    _log_to_file(tmp_path / "sliced.rrd", rr.Points3D(sliced))

    assert _position_rows(tmp_path / "sliced.rrd") == [[3.0, 4.0, 5.0], [6.0, 7.0, 8.0]]


@pytest.mark.parametrize("labels", [["a"], ["a", "bcd", ""]], ids=["one", "several"])
def test_string_columns_are_logged_correctly(labels: list[str], tmp_path: Path) -> None:
    positions = np.arange(3 * len(labels), dtype=np.float32).reshape(-1, 3)

    _log_to_file(tmp_path / "labels.rrd", rr.Points3D(positions, labels=labels))

    got: list[str] = []
    for chunk in RrdReader(tmp_path / "labels.rrd").stream().to_chunks():
        if chunk.entity_path != "/e":
            continue
        batch = chunk.to_record_batch()
        for name in batch.schema.names:
            if name.endswith(":labels"):
                for cell in batch.column(name).to_pylist():
                    got.extend(cell)
    assert got == labels
