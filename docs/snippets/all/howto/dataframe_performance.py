"""Sample snippets highlighting common performance-related improvements"""

import tempfile
from pathlib import Path

import pyarrow as pa
from datafusion import col

import rerun as rr

TMP_FILE = tempfile.NamedTemporaryFile(suffix=".rrd")
RRD_PATH = TMP_FILE.name

# region: get_df
sample_video_path = (
    Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "video_sample"
)
server = rr.server.Server(datasets={"video_dataset": sample_video_path})
# Using OSS server for demonstration but in practice replace with
# the URL of your cloud instance
CATALOG_URL = server.url()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="video_dataset")
df = dataset.filter_contents([
    "/compressed_images/**",
    "/raw_images/**",
]).reader(index="log_time")
# endregion: get_df

# region: to_list_bad
table = pa.table(df)
table["log_time"].to_numpy()
# vs.
table["log_time"].to_pylist()
# endregion: to_list_bad

# region: cache
df.count()  # has to pull some data
df.count()  # has to pull same data again
# vs.
cache_df = df.cache()  # materializes table in memory
cache_df.count()  # basically free
cache_df.count()  # basically free
# endregion: cache

# region: sparsity
# Create a new sparse layer identifying interesting events
segment_id = dataset.segment_ids()[0]
second_to_last_timestamp = pa.table(df)["log_time"].to_numpy()[-2]
with rr.RecordingStream("rerun_example_layer", recording_id=segment_id) as rec:
    rec.save(RRD_PATH)
    rec.set_log_time_enabled(False)
    rec.set_time("log_time", timestamp=second_to_last_timestamp)
    rec.log("/events", rr.AnyValues(flag=True))

dataset.register([Path(RRD_PATH).as_uri()], layer_name="event_layer")

# Step 1: read the marker on its own, which fetches no image chunks
events = (
    dataset
    .filter_contents("/events")
    .reader(index="log_time")
    .select("rerun_segment_id", "log_time")
)

# Step 2: fetch the images only at the marker's timestamps
images_at_events = dataset.filter_contents([
    "/compressed_images/**",
    "/raw_images/**",
]).reader(index="log_time", using_index_values=events)

# vs. one view with the marker and the images, which fetches every image chunk
df_with_flag = dataset.filter_contents([
    "/compressed_images/**",
    "/raw_images/**",
    "/events/**",
]).reader(index="log_time")
df_with_flag.filter(col("/events:flag").is_not_null())
# endregion: sparsity
