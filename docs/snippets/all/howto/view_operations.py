# region: setup
from __future__ import annotations

from pathlib import Path

import pyarrow as pa
import rerun as rr
from datafusion import col

sample_5_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "sample_5"

server = rr.server.Server(datasets={"sample_dataset": sample_5_path})
CATALOG_URL = server.address()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="sample_dataset")
observations = dataset.filter_contents("/observation/**")
# endregion: setup

# region: filtering
episode = "ILIAD_50aee79f_2023_07_12_20h_55m_08s"
start = 1689220508
end = start + 5
filtered_view = dataset.filter_segments(episode).filter_contents("/observation/**")
filtered_df = filtered_view.reader(index="real_time")
filtered_df = filtered_df.filter((col("real_time") >= start) & (col("real_time") < end))
# endregion: filtering

# region: static_data
instructions = dataset.filter_contents("/language_instruction/**").reader(index=None)

# Sort to ensure documented output is always correct
instructions = instructions.sort("/language_instruction:TextDocument:text")
instructions_tbl = pa.table(instructions)
instructions_tbl["/language_instruction:TextDocument:text"][0]
# endregion: static_data
