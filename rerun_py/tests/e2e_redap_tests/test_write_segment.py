from __future__ import annotations

import io
from typing import TYPE_CHECKING

import pytest
import rerun as rr

if TYPE_CHECKING:
    from collections.abc import Iterable
    from pathlib import Path
    from typing import BinaryIO

    from .conftest import EntryFactory

pytestmark = pytest.mark.local_only


@pytest.mark.parametrize("source_kind", ["str", "path", "bytes", "reader", "iterable"])
def test_write_and_register(entry_factory: EntryFactory, tmp_path: Path, source_kind: str) -> None:
    path = tmp_path / "recording.rrd"
    recording_id = "01234567-0123-0123-0123-0123456789ab"
    with rr.RecordingStream("rerun_example_write", recording_id=recording_id) as recording:
        recording.save(path)
        recording.log("points", rr.Points2D([[1, 2]]))

    source: str | Path | bytes | BinaryIO | Iterable[bytes]
    if source_kind == "str":
        source = str(path)
    elif source_kind == "path":
        source = path
    elif source_kind == "bytes":
        source = path.read_bytes()
    elif source_kind == "reader":
        source = io.BytesIO(path.read_bytes())
    else:
        data = path.read_bytes()
        source = iter([data[: len(data) // 2], data[len(data) // 2 :]])

    dataset = entry_factory.create_dataset("write")
    assert dataset.segment_ids() == []

    storage_prefix = f"write/{source_kind}"
    operation = dataset._write_segments(source, storage_prefix=storage_prefix)
    result = operation.wait(timeout_secs=None if source_kind == "str" else 30)
    assert result.segment_ids == [recording_id]
    assert f"/{storage_prefix}/" in result.storage_url
    assert operation.status() == "succeeded"

    path.unlink()
    assert dataset.segment_ids() == [recording_id]
    assert operation.wait() == result


@pytest.mark.parametrize("storage_prefix", ["", "..", "/write"])
def test_write_invalid_prefix(entry_factory: EntryFactory, storage_prefix: str) -> None:
    dataset = entry_factory.create_dataset("write_invalid_prefix")
    with pytest.raises(ValueError, match="invalid storage_prefix"):
        dataset._write_segments(io.BytesIO(b"data"), storage_prefix=storage_prefix)
    assert dataset.segment_ids() == []


def test_write_invalid_rrd_fails_terminally(entry_factory: EntryFactory) -> None:
    dataset = entry_factory.create_dataset("write_invalid_rrd")
    operation = dataset._write_segments(b"not an rrd")
    with pytest.raises(RuntimeError, match="failed to register uploaded RRD") as first_error:
        operation.wait(timeout_secs=30)
    assert operation.status() == "failed"
    with pytest.raises(RuntimeError, match="failed to register uploaded RRD") as second_error:
        operation.wait(timeout_secs=30)
    assert str(second_error.value) == str(first_error.value)
    assert "Dataset:" in str(first_error.value)
    assert "Staged URL:" in str(first_error.value)


def test_write_missing_file(entry_factory: EntryFactory, tmp_path: Path) -> None:
    dataset = entry_factory.create_dataset("write_missing_file")
    operation = dataset._write_segments(tmp_path / "missing.rrd")
    with pytest.raises(OSError, match="failed to open source"):
        operation.wait(timeout_secs=30)
    assert operation.status() == "failed"
    assert dataset.segment_ids() == []
