from __future__ import annotations

from typing import TYPE_CHECKING

from datafusion import col

if TYPE_CHECKING:
    from .conftest import ServerInstance


def test_component_filtering(server_instance: ServerInstance) -> None:
    """
    Cover the case where a user specifies a component filter on the client.

    We also support push down filtering to take a `.filter()` on the dataframe gets
    pushed into the query. Verify these both give the same results and that we don't
    get any nulls in that column.
    """
    dataset = server_instance.dataset

    component_path = "/obj2:Points3D:positions"

    filter_on_query = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .filter_is_not_null(component_path)
        .df()
        .collect_partitioned()
    )

    filter_on_dataframe = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .df()
        .filter(col(component_path).is_not_null())
        .collect_partitioned()
    )

    for outer in filter_on_dataframe:
        for inner in outer:
            column = inner.column(component_path)
            assert column.null_count == 0

    assert filter_on_query == filter_on_dataframe


def test_partition_ordering(server_instance: ServerInstance) -> None:
    dataset = server_instance.dataset

    for time_index in ["time_1", "time_2", "time_3"]:
        streams = (
            dataset.dataframe_query_view(index=time_index, contents="/**")
            .fill_latest_at()
            .df()
            .select("rerun_partition_id", time_index)
            .execute_stream_partitioned()
        )

        prior_partition_ids = set()
        for rb_reader in streams:
            prior_partition = ""
            prior_timestamp = 0
            for rb in iter(rb_reader):
                rb = rb.to_pyarrow()
                for idx in range(rb.num_rows):
                    partition = rb[0][idx].as_py()

                    # Nanosecond timestamps cannot be converted using `as_py()`
                    timestamp = rb[1][idx]
                    timestamp = timestamp.value if hasattr(timestamp, "value") else timestamp.as_py()

                    assert partition >= prior_partition
                    if partition == prior_partition and timestamp is not None:
                        assert timestamp >= prior_timestamp
                    else:
                        assert partition not in prior_partition_ids
                        prior_partition_ids.add(partition)

                    prior_partition = partition
                    if timestamp is not None:
                        prior_timestamp = timestamp


def test_tables_to_arrow_reader(server_instance: ServerInstance) -> None:
    dataset = server_instance.dataset

    for rb in dataset.dataframe_query_view(index="time_1", contents="/**").to_arrow_reader():
        assert rb.num_rows > 0

    for partition_batch in dataset.partition_table().to_arrow_reader():
        assert partition_batch.num_rows > 0

    for table_entry in server_instance.client.table_entries()[0].to_arrow_reader():
        assert table_entry.num_rows > 0


def test_query_view_from_schema(server_instance: ServerInstance) -> None:
    """Verify Our Schema is sufficiently descriptive to extract all contents from dataset."""
    from rerun_bindings import IndexColumnDescriptor

    dataset = server_instance.dataset

    # TODO(nick): This only works for a single shared index column
    # We should consider if our schema is sufficiently descriptive for
    # multi-indices
    index_column = None
    for entry in dataset.schema():
        if isinstance(entry, IndexColumnDescriptor):
            index_column = entry.name
        else:
            local_index_column = index_column
            if entry.is_static:
                local_index_column = None
            contents = dataset.dataframe_query_view(
                index=local_index_column, contents={entry.entity_path: entry.component}
            ).df()
            assert contents.count() > 0


def test_dataset_schema_comparison_self_consistent(server_instance: ServerInstance) -> None:
    dataset = server_instance.dataset

    schema_0 = dataset.schema()
    schema_1 = dataset.schema()
    set_diff = set(schema_0).symmetric_difference(schema_1)

    assert len(set_diff) == 0, f"Schema iterator is not self-consistent: {set_diff}"
    assert schema_0 == schema_1, "Schema is not self-consistent"
