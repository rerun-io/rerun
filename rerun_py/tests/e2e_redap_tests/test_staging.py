from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import CatalogClient

    from .conftest import EntryFactory

pytestmark = pytest.mark.local_only


@pytest.mark.parametrize("source_kind", ["str", "path", "bytes"])
def test_stage_and_register(
    catalog_client: CatalogClient, entry_factory: EntryFactory, tmp_path: Path, source_kind: str
) -> None:
    path = tmp_path / "recording.rrd"
    recording_id = "01234567-0123-0123-0123-0123456789ab"
    with rr.RecordingStream("rerun_example_staging", recording_id=recording_id) as recording:
        recording.save(path)
        recording.log("points", rr.Points2D([[1, 2]]))

    source: str | Path | bytes
    if source_kind == "str":
        source = str(path)
    elif source_kind == "path":
        source = path
    else:
        source = path.read_bytes()

    dataset = entry_factory.create_dataset("staging")
    key = f"staging/{source_kind}/recording.rrd"
    uri = catalog_client.stage(source, key=key)
    assert dataset.segment_ids() == []

    with pytest.raises(RuntimeError, match="HTTP 409"):
        catalog_client.stage(source, key=key)

    path.unlink()
    result = dataset.register([uri]).wait(timeout_secs=30)
    assert result.segment_ids == [recording_id]
    assert dataset.segment_ids() == [recording_id]


@pytest.mark.parametrize("key", ["", "../recording.rrd", "/recording.rrd"])
def test_stage_invalid_key(catalog_client: CatalogClient, key: str) -> None:
    with pytest.raises(ValueError, match="invalid object key"):
        catalog_client.stage(b"data", key=key)


def test_stage_missing_file(catalog_client: CatalogClient, tmp_path: Path) -> None:
    with pytest.raises(OSError, match="failed to open source"):
        catalog_client.stage(tmp_path / "missing.rrd", key="staging/missing.rrd")
