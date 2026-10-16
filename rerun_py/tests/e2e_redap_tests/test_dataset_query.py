from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow
import pytest
import rerun as rr
from datafusion import col

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import DatasetEntry

    from .conftest import EntryFactory, PrefilledCatalog


def test_component_filtering(readonly_test_dataset: DatasetEntry) -> None:
    """
    Cover the case where a user specifies a component filter on the client.

    Verify that filtering for non-null values on a column works and that we don't
    get any nulls in that column.
    """

    component_path = "/obj2:Points3D:positions"

    filtered_rb = readonly_test_dataset.reader(index="time_1").filter(col(component_path).is_not_null()).collect()

    for rb in filtered_rb:
        column = rb.column(component_path)
        assert column.null_count == 0


def test_segment_ordering(readonly_test_dataset: DatasetEntry) -> None:
    for time_index in ["time_1", "time_2", "time_3"]:
        streams = (
            readonly_test_dataset
            .reader(index=time_index, fill_latest_at=True)
            .select("rerun_segment_id", time_index)
            .execute_stream_partitioned()
        )

        prior_segment_ids = set()
        for rb_reader in streams:
            prior_segment = ""
            prior_timestamp = 0
            for rb in iter(rb_reader):
                rb_arrow: pyarrow.RecordBatch = rb.to_pyarrow()
                for idx in range(rb_arrow.num_rows):
                    segment = rb_arrow[0][idx].as_py()

                    # Nanosecond timestamps cannot be converted using `as_py()`
                    timestamp = rb_arrow[1][idx]
                    timestamp = timestamp.value if hasattr(timestamp, "value") else timestamp.as_py()

                    assert segment >= prior_segment
                    if segment == prior_segment and timestamp is not None:
                        assert timestamp >= prior_timestamp
                    else:
                        assert segment not in prior_segment_ids
                        prior_segment_ids.add(segment)

                    prior_segment = segment
                    if timestamp is not None:
                        prior_timestamp = timestamp


@pytest.mark.local_only
def test_static_overlay_layer_fills_every_row(entry_factory: EntryFactory, tmp_path: Path) -> None:
    """
    A static-only layer on a segment that also carries temporal chunks must reach every row.

    The two layers land in different chunks, so a query has to hold a row back until the
    static chunk has arrived. A row emitted before then carries a null where the static
    value belongs, which is wrong output rather than a failure — nothing raises, the
    column is simply empty for the rows that went out early.

    The local server plans this dataset as a single fetch batch, so the chunks arrive
    together and this cannot *force* the late-arrival ordering; that case is covered
    deterministically by `late_static_chunk_holds_back_every_row` in `re_datafusion`.
    What this test covers is the realistic segment shape end-to-end, over the wire.

    It runs under whichever pipeline `RERUN_DATAFRAME_PIPELINE` selects.
    """
    n_rows = 32
    base_path = tmp_path / "base.rrd"
    overlay_path = tmp_path / "statics.rrd"

    # One segment, two layers: the same recording id is what puts them in the same segment.
    with rr.RecordingStream("rerun_example_static_overlay", recording_id="segment1") as rec:
        rec.save(base_path)
        for t in range(n_rows):
            rec.set_time("frame", sequence=t)
            rec.log("/points", rr.Points2D([[float(t), float(t)]]))

    with rr.RecordingStream("rerun_example_static_overlay", recording_id="segment1") as rec:
        rec.save(overlay_path)
        rec.log("/meta", rr.AnyValues(static_tag=pyarrow.array([7], type=pyarrow.int64())), static=True)

    dataset = entry_factory.create_dataset("static_overlay")
    dataset.register([base_path.as_uri()], layer_name="base").wait()
    dataset.register([overlay_path.as_uri()], layer_name="statics").wait()

    batches = dataset.reader(index="frame", fill_latest_at=True).collect()
    assert batches, "query returned no batches"

    static_columns = [name for name in batches[0].schema.names if name.endswith("static_tag")]
    assert len(static_columns) == 1, f"expected exactly one static column, got {static_columns}"
    static_column = static_columns[0]

    total_rows = sum(batch.num_rows for batch in batches)
    null_rows = sum(batch.column(static_column).null_count for batch in batches)

    assert total_rows == n_rows
    assert null_rows == 0, f"{null_rows}/{total_rows} rows are missing the static value"


def test_dataset_to_arrow_reader(readonly_test_dataset: DatasetEntry) -> None:
    for rb_stream in readonly_test_dataset.reader(index="time_1").execute_stream():
        assert rb_stream.to_pyarrow().num_rows > 0

    segment_table = readonly_test_dataset.segment_table().to_arrow_table()
    assert segment_table.num_rows > 0


def test_tables_to_arrow_reader(prefilled_catalog: PrefilledCatalog) -> None:
    for table_entry in prefilled_catalog.prefilled_tables():
        assert pyarrow.Table.from_batches(table_entry.to_arrow_reader()).num_rows > 0


def test_query_view_from_schema(readonly_test_dataset: DatasetEntry) -> None:
    """Verify Our Schema is sufficiently descriptive to extract all contents from dataset."""
    from rerun.catalog import IndexColumnDescriptor

    # TODO(nick): This only works for a single shared index column
    # We should consider if our schema is sufficiently descriptive for
    # multi-indices
    index_column = None
    for entry in readonly_test_dataset.schema():
        if isinstance(entry, IndexColumnDescriptor):
            index_column = entry.name
        else:
            local_index_column = index_column
            if entry.is_static:
                local_index_column = None
            # Filter to specific entity/component using filter_contents with explicit path
            contents = readonly_test_dataset.filter_contents([entry.entity_path]).reader(
                index=local_index_column,
            )
            assert contents.count() > 0


def test_readonly_dataset_schema_comparison_self_consistent(readonly_test_dataset: DatasetEntry) -> None:
    schema_0 = readonly_test_dataset.schema()
    schema_1 = readonly_test_dataset.schema()
    set_diff = set(schema_0).symmetric_difference(schema_1)

    assert len(set_diff) == 0, f"Schema iterator is not self-consistent: {set_diff}"
    assert schema_0 == schema_1, "Schema is not self-consistent"
