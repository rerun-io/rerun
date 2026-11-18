"""Tests for `ChunkIndexingPolicy` validation, defaults, and client-side wildcard matching."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.catalog import ChunkIndexingPolicy

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import CatalogClient, DatasetEntry

    from e2e_redap_tests.conftest import EntryFactory


def create_dataset(entry_factory: EntryFactory, tmp_path: Path) -> DatasetEntry:
    rrd_path = tmp_path / "recording.rrd"
    with rr.RecordingStream("rerun_example_chunk_indexing_policy") as rec:
        rec.save(rrd_path)
        rec.set_time("sensor_time", sequence=0)
        rec.log("/world/car", rr.Points2D([[0, 0]]))
        rec.log("/world/raw/lidar", rr.Points2D([[1, 1]]))
        rec.log("/robot", rr.Points2D([[2, 2]]))
        rec.flush()

    ds = entry_factory.create_dataset("dataset")
    ds.register([rrd_path.absolute().as_uri()]).wait(timeout_secs=50)
    return ds


def test_invalid_policies_are_rejected(catalog_client: CatalogClient, entry_factory: EntryFactory) -> None:
    with pytest.raises(ValueError, match="/world/\\*\\*"):
        catalog_client.create_dataset(
            entry_factory.apply_prefix("dataset"),
            chunk_indexing_policy=ChunkIndexingPolicy(["/world/**"], []),
        )
    with pytest.raises(ValueError, match='"all"'):
        catalog_client.create_dataset(
            entry_factory.apply_prefix("dataset"),
            chunk_indexing_policy=ChunkIndexingPolicy("everything", []),  # type: ignore[arg-type]
        )

    ds = entry_factory.create_dataset("dataset")
    with pytest.raises(ValueError, match="/world/\\*\\*"):
        ds.set_chunk_indexing_policy(ChunkIndexingPolicy(["/world/**"], []))
    with pytest.raises(ValueError, match="-/robot"):
        ds.set_chunk_indexing_policy(ChunkIndexingPolicy(["-/robot"], []))


def test_default_policy_is_everything_and_can_be_reset(entry_factory: EntryFactory) -> None:
    ds = entry_factory.create_dataset("dataset")
    assert ds.chunk_indexing_policy == ChunkIndexingPolicy("all", "all")

    policy = ChunkIndexingPolicy(["/robot"], ["frame_nr"])
    ds.set_chunk_indexing_policy(policy).wait(timeout_secs=50)
    assert ds.chunk_indexing_policy == policy

    one_timeline = ChunkIndexingPolicy("all", ["frame_nr"])
    ds.set_chunk_indexing_policy(one_timeline).wait(timeout_secs=50)
    assert ds.chunk_indexing_policy == one_timeline

    ds.set_chunk_indexing_policy(None).wait(timeout_secs=50)
    assert ds.chunk_indexing_policy == ChunkIndexingPolicy("all", "all")


@pytest.mark.local_only
def test_match_against_schema(entry_factory: EntryFactory, tmp_path: Path) -> None:
    ds = create_dataset(entry_factory, tmp_path)

    policy = ChunkIndexingPolicy(["/world/**", "-/world/raw/**", "/not/yet/logged"], ["sensor_time"])
    matched = policy.match(ds.schema())

    assert matched == ChunkIndexingPolicy(["/not/yet/logged", "/world/car"], ["sensor_time"])
    ds.set_chunk_indexing_policy(matched).wait(timeout_secs=50)

    all_entity_paths = ChunkIndexingPolicy("all", ["sensor_time"])
    assert all_entity_paths.match(ds.schema()) == all_entity_paths
