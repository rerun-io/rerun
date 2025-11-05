from __future__ import annotations

import re
from typing import TYPE_CHECKING

import pyarrow as pa
from datafusion import col, functions as f

if TYPE_CHECKING:
    from .conftest import ServerInstance


def test_urls(server_instance: ServerInstance) -> None:
    """Tests the url property on the catalog and dataset."""

    catalog = server_instance.dataset.catalog
    assert re.match("^rerun\\+http://(localhost|127.0.0.1):[0-9]+$", catalog.url)

    table = server_instance.client.get_table_entry(name="simple_datatypes")
    assert re.match("^file:///[-_./0-9a-zA-Z]+/simple_datatypes/$", table.url)


def test_df_count(server_instance: ServerInstance) -> None:
    """
    Tests count() on a dataframe which ensures we collect empty batches properly.

    See issue https://github.com/rerun-io/rerun/issues/10894 for additional context.
    """
    dataset = server_instance.dataset

    count = dataset.dataframe_query_view(index="time_1", contents="/**").df().count()

    assert count > 0


def test_df_aggregation(server_instance: ServerInstance) -> None:
    dataset = server_instance.dataset

    results = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .df()
        .unnest_columns("/obj1:Points3D:positions")
        .aggregate(
            [],
            [
                f.min(col("/obj1:Points3D:positions")[0]).alias("min_x"),
                f.max(col("/obj1:Points3D:positions")[0]).alias("max_x"),
            ],
        )
        .collect()
    )

    assert results[0][0][0] == pa.scalar(1.0, type=pa.float32())
    assert results[0][1][0] == pa.scalar(50.0, type=pa.float32())
