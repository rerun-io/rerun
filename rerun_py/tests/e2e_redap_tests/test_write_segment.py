from __future__ import annotations

import io
from typing import TYPE_CHECKING

import pytest
import rerun as rr
import rerun.blueprint as rrb
from rerun.catalog import OnDuplicateSegmentLayer

if TYPE_CHECKING:
    from collections.abc import Iterable
    from pathlib import Path
    from typing import BinaryIO

    from .conftest import EntryFactory

pytestmark = [
    pytest.mark.requires_write_access_grants,
    pytest.mark.parametrize("write_kind", ["segments", "blueprint", "asset"]),
]


@pytest.mark.parametrize("source_kind", ["str", "path", "bytes", "bytearray", "memoryview", "reader", "iterable"])
@pytest.mark.parametrize("storage_prefix", [None, "write/nested"])
def test_write_and_register(
    entry_factory: EntryFactory, tmp_path: Path, source_kind: str, write_kind: str, storage_prefix: str | None
) -> None:
    extension = "rbl" if write_kind == "blueprint" else "rrd"
    path = tmp_path / f"recording.{extension}"
    recording_id = "01234567-0123-0123-0123-0123456789ab"
    if write_kind == "blueprint":
        rrb.Blueprint(rrb.Spatial2DView()).save("rerun_example_write", path)
    else:
        with rr.RecordingStream("rerun_example_write", recording_id=recording_id) as recording:
            recording.save(path)
            recording.log("points", rr.Points2D([[1, 2]]), static=write_kind == "asset")

    source: str | Path | bytes | bytearray | memoryview | BinaryIO | Iterable[bytes]
    if source_kind == "str":
        source = str(path)
    elif source_kind == "path":
        source = path
    elif source_kind == "bytes":
        source = path.read_bytes()
    elif source_kind == "bytearray":
        source = bytearray(path.read_bytes())
    elif source_kind == "memoryview":
        source = memoryview(path.read_bytes())
    elif source_kind == "reader":
        source = io.BytesIO(path.read_bytes())
    else:
        data = path.read_bytes()
        source = iter([data[: len(data) // 2], data[len(data) // 2 :]])

    dataset = entry_factory.create_dataset("write")
    assert dataset.segment_ids() == []

    write = getattr(dataset, f"_write_{write_kind}")
    operation = write(source, storage_prefix=storage_prefix, layer_name="custom")
    result = operation.wait(timeout_secs=None if source_kind == "str" else 30)
    if write_kind == "blueprint":
        assert len(result.segment_ids) == 1
        target = dataset.blueprint_dataset()
        assert dataset.default_blueprint() is None
        assert dataset.default_segment_table_blueprint() is None
    elif write_kind == "asset":
        assert result.segment_ids == [recording_id]
        target = dataset.asset_dataset()
        assert dataset.assets_for_segment("any_segment") == [recording_id]
    else:
        assert result.segment_ids == [recording_id]
        target = dataset

    if storage_prefix is not None:
        assert f"/{storage_prefix}/" in result.storage_url
    assert result.storage_url.endswith(f".{extension}")
    assert operation.status() == "succeeded"

    payload = path.read_bytes()
    path.unlink()
    assert target is not None
    assert target.segment_ids() == result.segment_ids
    assert target.segment(result.segment_ids[0]).layers() == {"custom"}
    if write_kind != "segments":
        assert dataset.segment_ids() == []
    assert operation.wait() == result

    if source_kind == "bytes":
        skipped = write(payload, layer_name="custom", on_duplicate=OnDuplicateSegmentLayer.SKIP).wait()
        assert skipped.segment_ids == result.segment_ids
        assert skipped.storage_url != result.storage_url
        replaced = write(payload, layer_name="custom", on_duplicate=OnDuplicateSegmentLayer.REPLACE).wait()
        assert replaced.segment_ids == result.segment_ids
        assert replaced.storage_url not in (result.storage_url, skipped.storage_url)


@pytest.mark.parametrize("storage_prefix", ["", "..", "/write"])
def test_write_invalid_prefix(entry_factory: EntryFactory, storage_prefix: str, write_kind: str) -> None:
    dataset = entry_factory.create_dataset("write_invalid_prefix")
    with pytest.raises(ValueError, match="invalid storage_prefix"):
        getattr(dataset, f"_write_{write_kind}")(io.BytesIO(b"data"), storage_prefix=storage_prefix)
    assert dataset.segment_ids() == []


def test_write_invalid_rrd_fails_terminally(entry_factory: EntryFactory, write_kind: str) -> None:
    dataset = entry_factory.create_dataset("write_invalid_rrd")
    operation = getattr(dataset, f"_write_{write_kind}")(b"not an rrd")
    with pytest.raises(ValueError, match="failed to register uploaded RRD") as first_error:
        operation.wait(timeout_secs=30)
    assert operation.status() == "failed"
    with pytest.raises(ValueError, match="failed to register uploaded RRD") as second_error:
        operation.wait(timeout_secs=30)
    assert str(second_error.value) == str(first_error.value)
    assert "Dataset:" in str(first_error.value)
    assert "Staged URL:" in str(first_error.value)


def test_write_missing_file(entry_factory: EntryFactory, tmp_path: Path, write_kind: str) -> None:
    dataset = entry_factory.create_dataset("write_missing_file")
    operation = getattr(dataset, f"_write_{write_kind}")(tmp_path / "missing.rrd")
    with pytest.raises(OSError, match="failed to open source"):
        operation.wait(timeout_secs=30)
    assert operation.status() == "failed"
    assert dataset.segment_ids() == []
