from __future__ import annotations

from typing import TYPE_CHECKING

from datafusion import col

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry


def test_url_generation(readonly_test_dataset: DatasetEntry) -> None:
    from rerun.utilities.datafusion.functions import url_generation

    udf = url_generation.segment_url_with_timeref_udf(readonly_test_dataset, "time_1")

    results = (
        readonly_test_dataset.reader(index="time_1")
        .with_column("url", udf(col("rerun_segment_id"), col("time_1")))
        .sort(col("rerun_segment_id"), col("time_1"))
        .limit(1)
        .select("url")
        .collect()
    )

    # Since the OSS server will generate a random dataset ID at startup, we can only check part of
    # the generated URL
    assert (
        "segment_id=141a866deb2d49f69eb3215e8a404ffc#when=time_1@2024-01-15T10:30:45.123457000Z"
        in results[0][0][0].as_py()
    )
