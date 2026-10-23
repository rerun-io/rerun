from __future__ import annotations

from datetime import UTC, datetime
from typing import TYPE_CHECKING
from uuid import uuid4

import pyarrow as pa
import pytest
import requests
import rerun as rr
from rerun.catalog import AlreadyExistsError

from rerun_bindings import _get_write_access_grant  # noqa: TID251

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import CatalogClient

    from .conftest import EntryFactory


@pytest.mark.requires_write_access_grants
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
    key = f"staging/{uuid4().hex}/{source_kind}/recording.rrd"
    uri = catalog_client.stage(source, key=key)
    assert dataset.segment_ids() == []

    with pytest.raises(AlreadyExistsError, match="object already exists"):
        catalog_client.stage(source, key=key)

    path.unlink()
    result = dataset.register([uri]).wait(timeout_secs=30)
    assert result.segment_ids == [recording_id]
    assert dataset.segment_ids() == [recording_id]


@pytest.mark.requires_write_access_grants
def test_write_access_grants_prevent_overwrite(entry_factory: EntryFactory, tmp_path: Path) -> None:
    client = entry_factory.client
    key = f"staging/{uuid4().hex}/recording.rrd"
    recording_id = str(uuid4())
    path = tmp_path / "recording.rrd"
    with rr.RecordingStream("rerun_example_conditional_upload", recording_id=recording_id) as recording:
        recording.save(path)
        recording.set_time("step", sequence=0)
        recording.log("value", rr.Scalars(42.0))

    first = _get_write_access_grant(client._internal, key=key)
    second = _get_write_access_grant(client._internal, key=key)
    storage_url = first[0]
    assert storage_url == second[0]
    for grant, payload, expected_status in (
        (first, path.read_bytes(), 200),
        (second, b"overwrite", 412),
    ):
        _, _, method, url, header_pairs = grant
        assert method == "PUT"
        headers = dict(header_pairs)
        assert headers["if-none-match"] == b"*"
        with requests.request(
            method, url, headers=headers, data=payload, timeout=30, allow_redirects=False
        ) as response:
            assert response.status_code == expected_status

    with pytest.raises(AlreadyExistsError, match="object already exists"):
        _get_write_access_grant(client._internal, key=key)

    # Register after both PUTs so readback cannot use data cached before the attempted overwrite.
    dataset = entry_factory.create_dataset(f"conditional_upload_{uuid4().hex}")
    result = dataset.register([storage_url]).wait(timeout_secs=30)
    assert result.segment_ids == [recording_id]
    values = pa.Table.from_batches(dataset.reader(index="step").select("/value:Scalars:scalars").collect())
    assert values.column(0).to_pylist() == [[42.0]]
    # TODO(RR-6076): Delete staged objects once staging exposes cleanup; until then use lifecycle expiry.


@pytest.mark.requires_write_access_grants
def test_get_write_access_grant(catalog_client: CatalogClient) -> None:
    key = f"staging/{uuid4().hex}/recording.rrd"
    first = _get_write_access_grant(catalog_client._internal, key=key)
    second = _get_write_access_grant(catalog_client._internal, key=key)

    assert first[0] == second[0]
    for _, expires_at, method, url, _ in (first, second):
        assert method == "PUT"
        assert url.startswith(("http://", "https://"))
        assert datetime.fromisoformat(expires_at) > datetime.now(UTC)


@pytest.mark.requires_write_access_grants
@pytest.mark.parametrize("key", ["", "../recording.rrd", "/recording.rrd"])
def test_get_write_access_grant_invalid_key(catalog_client: CatalogClient, key: str) -> None:
    with pytest.raises(ValueError, match="invalid object key"):
        _get_write_access_grant(catalog_client._internal, key=key)


@pytest.mark.requires_write_access_grants
@pytest.mark.parametrize("key", ["", "../recording.rrd", "/recording.rrd"])
def test_stage_invalid_key(catalog_client: CatalogClient, key: str) -> None:
    with pytest.raises(ValueError, match="invalid object key"):
        catalog_client.stage(b"data", key=key)


@pytest.mark.requires_write_access_grants
def test_stage_missing_file(catalog_client: CatalogClient, tmp_path: Path) -> None:
    with pytest.raises(OSError, match="failed to open source"):
        catalog_client.stage(tmp_path / "missing.rrd", key="staging/missing.rrd")
