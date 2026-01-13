# region: setup
from __future__ import annotations

from fractions import Fraction
from io import BytesIO
from pathlib import Path

import av
import numpy as np
import pyarrow as pa
import rerun as rr
from datafusion import col, functions as F
from datafusion.expr import Window, WindowFrame
from PIL import Image

sample_video_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "video_sample"

server = rr.server.Server(datasets={"video_dataset": sample_video_path})
CATALOG_URL = server.address()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="video_dataset")
df = dataset.filter_contents(["/compressed_images/**", "/raw_images/**", "/video_stream/**"]).reader(index="log_time")
times = pa.table(df.select("log_time"))["log_time"].to_numpy()
# endregion: setup

# region: compressed_image
column_name = "/compressed_images:EncodedImage:blob"
row = df.filter(col("log_time") == times[0]).select(column_name)
image_byte_array = pa.table(row)[column_name].to_numpy()[0][0]
image = np.asarray(Image.open(BytesIO(image_byte_array.tobytes())))
print(f"{image.shape=}")
# endregion: compressed_image

# region: raw_image
content_column = "/raw_images:Image:buffer"
format_column = "/raw_images:Image:format"
row = df.filter(col("log_time") == times[0]).select(content_column, format_column)
table = pa.table(row)
format_details = table[format_column][0][0]
flattened_image = table[content_column].to_numpy()[0][0]
num_channels = rr.datatypes.color_model.ColorModel.auto(int(format_details["color_model"].as_py())).num_channels()
image = flattened_image.reshape(format_details["height"].as_py(), format_details["width"].as_py(), num_channels)
print(f"{image.shape=}")
# endregion: raw_image

# region: video_stream
keyframe_column = "/video_stream:is_keyframe"
video_column = "/video_stream:VideoStream:sample"
video_df = df.select("log_time", keyframe_column, video_column)
selected_time = times[3]  # arbitrary

# Make sure keyframe is in the past
nearest_keyframe = video_df.filter((col("log_time") <= selected_time) & col(keyframe_column).is_not_null())

# Get the most recent keyframe timestamp
nearest_keyframe = nearest_keyframe.select(
    F.last_value(col("log_time"))
    .over(Window(window_frame=WindowFrame("rows", None, 0), order_by="log_time"))
    .alias("log_time")
)
keyframe_ts = pa.table(nearest_keyframe.select("log_time"))["log_time"].to_numpy()[0]

df = video_df.filter((col("log_time") <= selected_time) | (col("log_time") >= keyframe_ts))
rows = df.select("log_time", video_column)
pa_table = pa.table(rows)

samples = pa_table[video_column].to_numpy()
times = pa_table["log_time"].to_numpy()
sample_bytes = b""
for sample in samples:
    sample_bytes += sample[0].tobytes()
data_buffer = BytesIO(sample_bytes)

# We use av to decode on CPU, but could use any suitable
# video tool
container = av.open(data_buffer, format="h264", mode="r")

start_time = times[0]
video_stream = container.streams.video[0]
frame = None
for packet, time in zip(container.demux(video_stream), times, strict=False):
    packet.time_base = Fraction(1, 1_000_000_000)  # Assuming duration timestamps in nanoseconds.
    packet.pts = int(time - start_time)
    packet.dts = packet.pts  # dts == pts since there's no B-frames.
    for _idx, frame in enumerate(packet.decode()):  # noqa: B007
        pass
if frame is None:
    raise RuntimeError("Failed to decode any frame from video stream.")
image = np.asarray(frame.to_image())
print(f"{image.shape=}")
# endregion: video_stream
