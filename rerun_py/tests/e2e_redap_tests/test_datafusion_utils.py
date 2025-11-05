from __future__ import annotations

from typing import TYPE_CHECKING

from datafusion import col

if TYPE_CHECKING:
    from .conftest import ServerInstance


def test_url_generation(server_instance: ServerInstance) -> None:
    from rerun.utilities.datafusion.functions import url_generation

    dataset = server_instance.dataset

    udf = url_generation.partition_url_with_timeref_udf(dataset, "time_1")

    results = (
        dataset.dataframe_query_view(index="time_1", contents="/**")
        .df()
        .with_column("url", udf(col("rerun_partition_id"), col("time_1")))
        .sort(col("rerun_partition_id"), col("time_1"))
        .limit(1)
        .select("url")
        .collect()
    )

    # Since the OSS server will generate a random dataset ID at startup, we can only check part of
    # the generated URL
    assert (
        "partition_id=141a866deb2d49f69eb3215e8a404ffc#when=time_1@2024-01-15T10:30:45.123457000Z"
        in results[0][0][0].as_py()
    )
