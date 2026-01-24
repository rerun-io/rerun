"""Query various image representations."""

# region: setup
from __future__ import annotations

from io import BytesIO
from pathlib import Path

import numpy as np
import pyarrow as pa
from datafusion import col
from PIL import Image

import rerun as rr

sample_video_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "video_sample"

server = rr.server.Server(datasets={"video_dataset": sample_video_path})
CATALOG_URL = server.url()
client = server.client()
dataset = client.get_dataset(name="video_dataset")
df = dataset.filter_contents(["/compressed_images/**", "/raw_images/**"]).reader(index="log_time")
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
