"""Tests for the `scope` parameter on dataset schemas and readers."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.catalog import DatasetQueryScope

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import DatasetEntry

    from e2e_redap_tests.conftest import EntryFactory


def create_dataset(entry_factory: EntryFactory, tmp_path: Path) -> DatasetEntry:
    rrd_path = tmp_path / "recording.rrd"
    with rr.RecordingStream("rerun_example_dataset_scope") as rec:
        rec.save(rrd_path)
        rec.set_time("frame_nr", sequence=0)
        rec.log("/robot/speed", rr.Scalars(1.0))
        rec.set_time("frame_nr", sequence=1)
        rec.log("/camera/points", rr.Points2D([[0, 0]]))
        rec.flush()

    ds = entry_factory.create_dataset("dataset")
    ds.register([rrd_path.absolute().as_uri()]).wait(timeout_secs=50)
    return ds


# The OSS server ignores the chunk indexing policy, so both scopes cover the same data.
@pytest.mark.local_only
def test_scopes_without_policy(entry_factory: EntryFactory, tmp_path: Path) -> None:
    ds = create_dataset(entry_factory, tmp_path)

    assert ds.schema(scope=DatasetQueryScope.INDEXED).to_arrow() == ds.schema().to_arrow()
    assert ds.filter_contents("/robot/**").arrow_schema(scope=DatasetQueryScope.INDEXED) == ds.filter_contents(
        "/robot/**"
    ).arrow_schema(scope=DatasetQueryScope.FULL)

    full = ds.reader(index="frame_nr").to_arrow_table()
    indexed = ds.reader(index="frame_nr", scope=DatasetQueryScope.INDEXED).to_arrow_table()
    assert indexed.sort_by("frame_nr") == full.sort_by("frame_nr")
    assert full.num_rows == 2

    assert ds.schema(scope="indexed").to_arrow() == ds.schema(scope=DatasetQueryScope.INDEXED).to_arrow()  # type: ignore[arg-type]

    with pytest.raises(ValueError, match="invalid scope"):
        ds.schema(scope="everything")  # type: ignore[arg-type]


@pytest.mark.local_only
def test_unknown_names_are_not_errors(entry_factory: EntryFactory, tmp_path: Path) -> None:
    ds = create_dataset(entry_factory, tmp_path)

    rows = ds.filter_contents("/does/not/exist").reader(index="frame_nr", scope=DatasetQueryScope.INDEXED).count()
    assert rows == 0

    with pytest.raises(ValueError, match="does not exist"):
        ds.reader(index="no_such_timeline", scope=DatasetQueryScope.INDEXED)
