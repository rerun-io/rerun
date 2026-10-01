"""Store the result of a catalog dataset query in a table."""

# region: setup
from __future__ import annotations

import tempfile
from pathlib import Path

import pyarrow as pa
from datafusion import col
from datafusion import functions as F

import rerun as rr

sample_5_path = (
    Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "sample_5"
)

temp_dir = tempfile.TemporaryDirectory()
storage_dir = Path(temp_dir.name)
server = rr.server.Server(datasets={"sample_dataset": sample_5_path})
CATALOG_URL = server.url()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="sample_dataset")
# endregion: setup

# region: query_dataset
observations = dataset.filter_contents([
    "/observation/joint_positions",
]).reader(index="real_time")

result = observations.aggregate(
    col("rerun_segment_id"),
    [
        F.min(col("real_time")).alias("first_observation"),
        F.max(col("real_time")).alias("last_observation"),
    ],
)
# endregion: query_dataset

# region: store_result
# A table index is optional and only needed for upserts. This query has one row
# per segment, so here we use the segment ID as each row's identity.
schema = pa.schema([
    field.with_metadata({rr.SORBET_IS_TABLE_INDEX: "true"})
    if field.name == "rerun_segment_id"
    else field
    for field in result.schema()
])

# A local server can optionally store the table in a specific directory.
# Omit the url argument when using Rerun Hub.
local_table_storage_url = (storage_dir / "catalog_storage").as_uri()

summary = client.create_table(
    "observation_summary",
    schema,
    url=local_table_storage_url,
)
result.write_table("observation_summary")
# endregion: store_result

# region: read_result
later_client = rr.catalog.CatalogClient(CATALOG_URL)
summary = later_client.get_table(name="observation_summary")
stored_result = summary.reader().sort(col("rerun_segment_id"))
stored_result.show()
# endregion: read_result

# region: update
batches = result.collect()

# Replace matching segments and append new ones.
summary.upsert(batches)
# endregion: update

# region: delete
summary.delete()
# endregion: delete
