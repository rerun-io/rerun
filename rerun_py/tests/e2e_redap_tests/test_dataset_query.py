from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow
from datafusion import col

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry

    from .conftest import PrefilledCatalog


def test_component_filtering(readonly_test_dataset: DatasetEntry) -> None:
    """
    Cover the case where a user specifies a component filter on the client.

    We also support push down filtering to take a `.filter()` on the dataframe gets
    pushed into the query. Verify these both give the same results and that we don't
    get any nulls in that column.
    """

    component_path = "/obj2:Points3D:positions"

    filter_on_query = (
        readonly_test_dataset.dataframe_query_view(index="time_1", contents="/**")
        .filter_is_not_null(component_path)
        .df()
        .collect_partitioned()
    )

    filter_on_dataframe = (
        readonly_test_dataset.dataframe_query_view(index="time_1", contents="/**")
        .df()
        .filter(col(component_path).is_not_null())
        .collect_partitioned()
    )

    for outer in filter_on_dataframe:
        for inner in outer:
            column = inner.column(component_path)
            assert column.null_count == 0

    assert filter_on_query == filter_on_dataframe


def test_segment_ordering(readonly_test_dataset: DatasetEntry) -> None:
    for time_index in ["time_1", "time_2", "time_3"]:
        streams = (
            readonly_test_dataset.dataframe_query_view(index=time_index, contents="/**")
            .fill_latest_at()
            .df()
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


def readonly_test_dataset_to_arrow_reader(readonly_test_dataset: DatasetEntry) -> None:
    for rb in readonly_test_dataset.dataframe_query_view(index="time_1", contents="/**").to_arrow_reader():
        assert rb.num_rows > 0

    for segment_batch in readonly_test_dataset.segment_table().to_arrow_reader():
        assert segment_batch.num_rows > 0


def test_tables_to_arrow_reader(prefilled_catalog: PrefilledCatalog) -> None:
    for table_entry in prefilled_catalog.client.tables():
        assert pyarrow.Table.from_batches(table_entry.to_arrow_reader()).num_rows > 0


def test_query_view_from_schema(readonly_test_dataset: DatasetEntry) -> None:
    """Verify Our Schema is sufficiently descriptive to extract all contents from dataset."""
    from rerun.dataframe import IndexColumnDescriptor

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
            contents = readonly_test_dataset.dataframe_query_view(
                index=local_index_column, contents={entry.entity_path: entry.component}
            ).df()
            assert contents.count() > 0


def test_readonly_dataset_schema_comparison_self_consistent(readonly_test_dataset: DatasetEntry) -> None:
    schema_0 = readonly_test_dataset.schema()
    schema_1 = readonly_test_dataset.schema()
    set_diff = set(schema_0).symmetric_difference(schema_1)

    assert len(set_diff) == 0, f"Schema iterator is not self-consistent: {set_diff}"
    assert schema_0 == schema_1, "Schema is not self-consistent"
