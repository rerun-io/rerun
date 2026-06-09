"""Roundtrip parity tests for `ChunkStore.reader()` vs. `dataset.reader()`."""

from __future__ import annotations

import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.experimental import ChunkStore, RrdReader

if TYPE_CHECKING:
    from collections.abc import Iterator

    import datafusion
    import pyarrow as pa
    from rerun.catalog import ContentFilter, DatasetEntry, IndexValuesLike


@dataclass(frozen=True)
class Case:
    """One roundtrip parity case."""

    name: str
    index: str | None
    contents: ContentFilter | str | list[str] | None = None
    include_semantically_empty_columns: bool = False
    include_tombstone_columns: bool = False
    fill_latest_at: bool = False
    using_index_values: IndexValuesLike | None = None

    def _common_kwargs(self) -> dict[str, object]:
        return {
            "index": self.index,
            "include_semantically_empty_columns": self.include_semantically_empty_columns,
            "include_tombstone_columns": self.include_tombstone_columns,
            "fill_latest_at": self.fill_latest_at,
            "using_index_values": self.using_index_values,
        }

    def chunk_df(self, store: ChunkStore) -> datafusion.DataFrame:
        return store.reader(contents=self.contents, **self._common_kwargs())  # type: ignore[arg-type]

    def dataset_df(self, ds: DatasetEntry) -> datafusion.DataFrame:
        view = ds.filter_contents(self.contents) if self.contents is not None else ds
        return view.reader(**self._common_kwargs())  # type: ignore[arg-type]


# Total non-static rows logged on timeline `t`. Sized above
# `DEFAULT_BATCH_ROWS=2048` so the batch-shape test sees multi-batch output.
FIXTURE_NUM_ROWS = 5000
FIXTURE_INDEX_RANGE = range(FIXTURE_NUM_ROWS)


def _build_fixture_store() -> ChunkStore:
    """
    `FIXTURE_NUM_ROWS` rows on timeline `t` across `/a` and `/b`, plus a static row on `/c`.

    Built via `RecordingStream` + `RrdReader.collect()` so the chunkification is
    whatever the standard SDK pipeline produces — no hand-crafted chunks.

    The static row is on `/c` (not `/a` or `/b`) so it doesn't collide with the
    temporal `Scalars:scalars` column on the same entity, which would mark the
    column as static and zero out the temporal rows.

    `/a` also gets a `rr.Clear` to produce a tombstone column, and `/q` logs
    `Points3D(positions=…, colors=[])` to produce a semantically-empty `colors`
    column. Both are hidden under the default reader and surface only when
    `include_tombstone_columns` / `include_semantically_empty_columns` is set.
    """
    with tempfile.TemporaryDirectory() as td:
        path = Path(td) / "build.rrd"
        with rr.RecordingStream("rerun_example_fixture", recording_id="fix") as rec:
            rec.save(path)
            rec.log("/c", rr.Scalars(scalars=[42.0]), static=True)
            for i in FIXTURE_INDEX_RANGE:
                rec.set_time("t", sequence=i)
                rec.log("/a", rr.Scalars(scalars=[float(i)]))
                if i % 2 == 0:
                    rec.log("/b", rr.Scalars(scalars=[float(-i)]))
            # Tombstone column: `Clear:is_recursive` on /a.
            rec.set_time("t", sequence=FIXTURE_NUM_ROWS // 2)
            rec.log("/a", rr.Clear(recursive=False))
            # Semantically-empty column: `/q:Points3D:colors` (positions logged,
            # colors logged as an explicit empty list — registers the column
            # with only null values).
            rec.set_time("t", sequence=0)
            rec.log("/q", rr.Points3D(positions=[[0.0, 0.0, 0.0]], colors=[]))
            rec.disconnect()

        return RrdReader(path).stream().collect()


@pytest.fixture(scope="module")
def store_and_dataset(
    tmp_path_factory: pytest.TempPathFactory,
) -> Iterator[tuple[ChunkStore, DatasetEntry]]:
    """
    Module-scoped server hosting a dataset registered from a single RRD.

    The same `ChunkStore` is yielded so both reader paths see the same data.
    """
    store = _build_fixture_store()
    rrd_dir = tmp_path_factory.mktemp("rt_dir")
    rrd = rrd_dir / "rt.rrd"
    store.write_rrd(rrd, application_id="rerun_example_test", recording_id="rt-rec")
    with rr.server.Server(datasets={"rt": rrd_dir}) as server:
        client = server.client()
        yield store, client.get_dataset("rt")


@pytest.fixture(scope="module")
def store_only() -> ChunkStore:
    """Standalone fixture for tests that don't need a server."""
    return _build_fixture_store()


# --- Helpers ---------------------------------------------------------------


def _drop_segment_id(df: datafusion.DataFrame) -> datafusion.DataFrame:
    return df.drop("rerun_segment_id")


def _normalized_fields(schema: pa.Schema) -> list[tuple[str, pa.DataType, dict[bytes, bytes]]]:
    return sorted([(f.name, f.type, dict(f.metadata or {})) for f in schema])


def _assert_field_parity(chunk_df: datafusion.DataFrame, dataset_df: datafusion.DataFrame) -> None:
    """Compare `(name, type, per-field metadata)` triplets sorted by name; ignore table-level metadata."""
    chunk_fields = _normalized_fields(chunk_df.schema())
    dataset_fields = _normalized_fields(_drop_segment_id(dataset_df).schema())
    assert chunk_fields == dataset_fields


def _row_multiset(df: datafusion.DataFrame) -> list[str]:
    """
    Convert every row to a deterministic Python `repr` and return sorted.

    Columns are read in alphabetical order so the two sides compare regardless
    of physical column ordering — the schema-parity contract only guarantees
    same fields (sorted by name), not same field order.

    `pyarrow.Table.sort_by` does not support List/Struct sort keys (which is
    every component column), so we cannot use a column-sort comparison.
    `to_pylist()` returns nested Python objects that `repr()` formats
    deterministically, so a sorted multiset of repr-strings is a robust
    row-set equality check. The fixture avoids NaN.
    """
    tbl = df.to_arrow_table().combine_chunks()
    names = sorted(tbl.column_names)
    cols = [tbl.column(n).to_pylist() for n in names]
    return sorted(repr(row) for row in zip(*cols, strict=True))


def _assert_data_parity(chunk_df: datafusion.DataFrame, dataset_df: datafusion.DataFrame) -> None:
    assert _row_multiset(chunk_df) == _row_multiset(_drop_segment_id(dataset_df))


# --- Parameterized roundtrip cases ----------------------------------------


# `using_index_values` entries must lie within FIXTURE_INDEX_RANGE so the
# dataset side's `_map_index_values_to_ranges` does not drop any value.
CASES: list[Case] = [
    Case("static_only", index=None),
    Case("timeline", index="t"),
    Case("narrow_contents", index="t", contents="/a/**"),
    Case("exclude_contents", index="t", contents=["/**", "-/b/**"]),
    Case("fill_latest_at", index="t", fill_latest_at=True),
    Case("using_index_values", index="t", using_index_values=[1, 2, 3]),
    Case("using_index_values_fill_latest_at", index="t", fill_latest_at=True, using_index_values=[5, 4999]),
    Case("include_tombstones", index="t", include_tombstone_columns=True),
    Case("include_semantically_empty", index="t", include_semantically_empty_columns=True),
]


@pytest.mark.parametrize("case", CASES, ids=[c.name for c in CASES])
def test_roundtrip_parity(
    store_and_dataset: tuple[ChunkStore, DatasetEntry],
    case: Case,
) -> None:
    store, ds = store_and_dataset

    ck_df = case.chunk_df(store)
    ds_df = case.dataset_df(ds)

    _assert_field_parity(ck_df, ds_df)
    assert ck_df.count() == ds_df.count()
    _assert_data_parity(ck_df, ds_df)


# --- Batch-shape ----------------------------------------------------------


def test_batch_shape(store_and_dataset: tuple[ChunkStore, DatasetEntry]) -> None:
    store, ds = store_and_dataset
    ck_batches = store.reader(index="t").collect()
    ds_batches = ds.reader(index="t").collect()

    ck_total = sum(b.num_rows for b in ck_batches)
    ds_total = sum(b.num_rows for b in ds_batches)
    assert ck_total == ds_total

    half = 2048 // 2
    if len(ck_batches) > 1:
        assert all(b.num_rows >= half for b in ck_batches[:-1])
    if len(ds_batches) > 1:
        assert all(b.num_rows >= half for b in ds_batches[:-1])

    # Sanity: fixture is large enough that we actually exercised the multi-batch path.
    assert len(ck_batches) >= 2, f"fixture too small: got {len(ck_batches)} batch(es)"


# --- Standalone (non-roundtrip) -------------------------------------------


def test_empty_contents_empty_result(store_only: ChunkStore) -> None:
    df = store_only.reader(index="t", contents=[])
    assert df.count() == 0


def test_unknown_index_errors(store_only: ChunkStore) -> None:
    with pytest.raises(Exception, match="does not exist"):
        store_only.reader(index="does_not_exist")


def test_include_tombstones_surfaces_clear_column(store_only: ChunkStore) -> None:
    """`include_tombstone_columns=True` must add the Clear:is_recursive column hidden by default."""
    default_cols = set(store_only.reader(index="t").schema().names)
    with_tombstones = set(store_only.reader(index="t", include_tombstone_columns=True).schema().names)
    added = with_tombstones - default_cols
    assert any("Clear" in c for c in added), f"expected a Clear:* column, got added={added}"


def test_include_semantically_empty_surfaces_null_column(store_only: ChunkStore) -> None:
    """`include_semantically_empty_columns=True` must add the all-null `/q:Points3D:colors` column."""
    default_cols = set(store_only.reader(index="t").schema().names)
    with_empty = set(store_only.reader(index="t", include_semantically_empty_columns=True).schema().names)
    added = with_empty - default_cols
    assert "/q:Points3D:colors" in added, f"expected /q:Points3D:colors, got added={added}"
