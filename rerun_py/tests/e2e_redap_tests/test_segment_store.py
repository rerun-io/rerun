from __future__ import annotations

from typing import TYPE_CHECKING, NamedTuple

import pytest
import rerun as rr
from rerun.catalog import DatasetEntry, NotFoundError
from rerun.chunk import LazyStore, RrdReader

if TYPE_CHECKING:
    from pathlib import Path

    from e2e_redap_tests.conftest import EntryFactory

# The asset and the segment are logged under different entity paths, so that a store makes it
# obvious which of the two it covers.
ASSET_ENTITY = "/shared/mesh"
SEGMENT_ENTITY = "/robot/pose"

# The id both the segment and the asset of `dataset_with_asset_sharing_segment_id` go by.
SHARED_SEGMENT_ID = "one_id_two_datasets"


@pytest.fixture(scope="module")
def first_segment_store(readonly_test_dataset: DatasetEntry) -> LazyStore:
    """The `LazyStore` for the first segment in [`readonly_test_dataset`][]."""
    segment_ids = readonly_test_dataset.segment_ids()
    assert len(segment_ids) > 0
    return readonly_test_dataset.segment_store(segment_ids[0])


@pytest.fixture
def single_segment_store(entry_factory: EntryFactory, resource_prefix: str) -> LazyStore:
    """A `LazyStore` over a freshly-registered dataset containing exactly one segment."""
    ds = entry_factory.create_dataset("single_segment")
    handle = ds.register([resource_prefix + "dataset/file1.rrd"])
    handle.wait(timeout_secs=50)
    segment_ids = ds.segment_ids()
    assert len(segment_ids) == 1
    return ds.segment_store(segment_ids[0])


class DatasetWithAsset(NamedTuple):
    dataset: DatasetEntry
    segment_id: str


def register_segment_and_asset(
    entry_factory: EntryFactory,
    tmp_path: Path,
    dataset_name: str,
    segment_id: str,
    asset_id: str,
) -> DatasetWithAsset:
    """A dataset holding one temporal segment plus one static asset, on distinct entity paths."""
    segment_rrd = tmp_path / "segment.rrd"
    with rr.RecordingStream("rerun_example_segment_store_assets", recording_id=segment_id) as rec:
        rec.save(segment_rrd)
        rec.set_time("frame", sequence=0)
        rec.log(SEGMENT_ENTITY, rr.Points2D([[0, 0]]))
        rec.flush()

    asset_rrd = tmp_path / "asset.rrd"
    with rr.RecordingStream("rerun_example_segment_store_assets", recording_id=asset_id) as rec:
        rec.save(asset_rrd)
        rec.log(ASSET_ENTITY, rr.Points2D([[1, 1]]), static=True)
        rec.flush()

    ds = entry_factory.create_dataset(dataset_name)
    ds.register([segment_rrd.absolute().as_uri()]).wait(timeout_secs=50)
    assert ds.register_asset(asset_rrd.absolute().as_uri()) == asset_id

    assert ds.segment_ids() == [segment_id]
    return DatasetWithAsset(ds, segment_id)


@pytest.fixture(scope="session")
def dataset_with_asset(
    session_entry_factory: EntryFactory, tmp_path_factory: pytest.TempPathFactory
) -> DatasetWithAsset:
    """A dataset holding one temporal segment plus one static asset, on distinct entity paths."""
    return register_segment_and_asset(
        session_entry_factory,
        tmp_path_factory.mktemp("dataset_with_asset"),
        "segment_store_with_asset",
        segment_id="episode",
        asset_id="shared_mesh",
    )


@pytest.fixture(scope="session")
def dataset_with_asset_sharing_segment_id(
    session_entry_factory: EntryFactory, tmp_path_factory: pytest.TempPathFactory
) -> DatasetWithAsset:
    """
    Like [`dataset_with_asset`][], but the asset carries the same segment id as the segment.

    Segment ids are only unique within a dataset, and the assets of a dataset live in their own
    asset dataset, so the two ids can be the same.
    """
    return register_segment_and_asset(
        session_entry_factory,
        tmp_path_factory.mktemp("dataset_with_asset_sharing_segment_id"),
        "segment_store_asset_sharing_segment_id",
        segment_id=SHARED_SEGMENT_ID,
        asset_id=SHARED_SEGMENT_ID,
    )


@pytest.mark.local_only
def test_segment_store_covers_assets(dataset_with_asset: DatasetWithAsset) -> None:
    """A segment store describes the dataset's assets alongside the segment's own data."""
    ds, segment_id = dataset_with_asset
    store = ds.segment_store(segment_id)

    assert set(store.schema().entity_paths()) == {SEGMENT_ENTITY, ASSET_ENTITY}
    assert ASSET_ENTITY in store.summary()


@pytest.mark.local_only
def test_segment_store_without_assets_leaves_them_out(dataset_with_asset: DatasetWithAsset) -> None:
    """Opting out of assets gives back a store describing only the segment."""
    ds, segment_id = dataset_with_asset
    store = ds.segment_store(segment_id, include_assets=False)

    assert set(store.schema().entity_paths()) == {SEGMENT_ENTITY}
    assert ASSET_ENTITY not in store.summary()
    assert len(store) < len(ds.segment_store(segment_id))


@pytest.mark.local_only
def test_segment_store_streams_asset_chunks(dataset_with_asset: DatasetWithAsset) -> None:
    """An asset's chunks live in another segment, so fetching them spans two datasets."""
    ds, segment_id = dataset_with_asset
    store = ds.segment_store(segment_id)

    streamed = {chunk.entity_path for chunk in store.stream().to_chunks()}
    assert {SEGMENT_ENTITY, ASSET_ENTITY} <= streamed

    without_assets = ds.segment_store(segment_id, include_assets=False)
    assert ASSET_ENTITY not in {chunk.entity_path for chunk in without_assets.stream().to_chunks()}


@pytest.mark.local_only
def test_segment_store_pushdown_reaches_assets(dataset_with_asset: DatasetWithAsset) -> None:
    """The asset holds the only static data logged here, so a static filter has to reach it."""
    ds, segment_id = dataset_with_asset
    store = ds.segment_store(segment_id)

    # Recording properties are static too, so the asset is not the only entity left standing.
    static_paths = {chunk.entity_path for chunk in store.stream().filter(is_static=True).to_chunks()}
    assert ASSET_ENTITY in static_paths
    assert SEGMENT_ENTITY not in static_paths


@pytest.mark.local_only
def test_segment_store_covers_asset_sharing_the_segment_id(
    dataset_with_asset_sharing_segment_id: DatasetWithAsset,
) -> None:
    """An asset that goes by the same segment id as the segment it is opened with is still its own data."""
    ds, segment_id = dataset_with_asset_sharing_segment_id
    assert ds.assets() == [segment_id]

    store = ds.segment_store(segment_id)

    assert set(store.schema().entity_paths()) == {SEGMENT_ENTITY, ASSET_ENTITY}
    streamed = {chunk.entity_path for chunk in store.stream().to_chunks()}
    assert {SEGMENT_ENTITY, ASSET_ENTITY} <= streamed


@pytest.mark.local_only
def test_segment_store_without_assets_sharing_the_segment_id(
    dataset_with_asset_sharing_segment_id: DatasetWithAsset,
) -> None:
    """Assets are left out by their manifest, so one going by the segment's id stays out too."""
    ds, segment_id = dataset_with_asset_sharing_segment_id
    store = ds.segment_store(segment_id, include_assets=False)

    assert set(store.schema().entity_paths()) == {SEGMENT_ENTITY}
    assert ASSET_ENTITY not in {chunk.entity_path for chunk in store.stream().to_chunks()}


def test_segment_store_basic(first_segment_store: LazyStore) -> None:
    assert isinstance(first_segment_store, LazyStore)
    assert len(first_segment_store) > 0
    paths = first_segment_store.schema().entity_paths()
    assert any(p.startswith("/obj") for p in paths), f"got {paths!r}"


def test_segment_store_summary_uses_manifest(first_segment_store: LazyStore) -> None:
    """`summary()` walks the manifest only — no chunk fetch."""
    summary = first_segment_store.summary()
    assert summary
    assert "rows=" in summary


def test_segment_store_stream_to_chunks(first_segment_store: LazyStore) -> None:
    chunks = first_segment_store.stream().to_chunks()
    assert len(chunks) > 0
    for chunk in chunks:
        assert chunk.num_rows > 0


def test_segment_store_write_rrd_roundtrip(single_segment_store: LazyStore, tmp_path: Path) -> None:
    """Round-trip a single segment through `write_rrd`: schema and chunk count are preserved."""
    out = tmp_path / "out.rrd"

    single_segment_store.stream().write_rrd(out, application_id="rerun_example_test", recording_id="rec")

    roundtripped = RrdReader(out).store()
    assert roundtripped.schema() == single_segment_store.schema()
    assert len(roundtripped) == len(single_segment_store)


def test_segment_store_unknown_segment_raises(readonly_test_dataset: DatasetEntry) -> None:
    """Unknown segment id surfaces synchronously at construction (eager manifest)."""
    with pytest.raises(NotFoundError, match=r"does-not-exist"):
        readonly_test_dataset.segment_store("does-not-exist")


def test_segment_store_compile_twice_works(first_segment_store: LazyStore) -> None:
    """Each `compile()` opens its own FetchChunks; same chunks both times."""
    stream = first_segment_store.stream()

    first = stream.to_chunks()
    second = stream.to_chunks()
    assert len(first) == len(second)
    assert {c.id for c in first} == {c.id for c in second}
