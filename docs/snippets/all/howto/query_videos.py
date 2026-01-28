"""Query video streams."""

# region: setup
from __future__ import annotations

from fractions import Fraction
from io import BytesIO
from pathlib import Path

import av
import numpy as np
import pyarrow as pa
import rerun as rr
from datafusion import col

sample_video_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "video_sample"

server = rr.server.Server(datasets={"video_dataset": sample_video_path})
CATALOG_URL = server.url()
client = rr.catalog.CatalogClient(CATALOG_URL)
dataset = client.get_dataset(name="video_dataset")
df = dataset.filter_contents(["/video_stream/**"]).reader(index="log_time")
times = pa.table(df.select("log_time"))["log_time"].to_numpy()
# endregion: setup

# region: check_codec
codec_column = "/video_stream:VideoStream:codec"
num_codec_matches = df.select(col(codec_column)[0] == rr.VideoCodec.H264.value).count()

if num_codec_matches != df.select(codec_column).count():
    raise ValueError(f"Expected H.264 codec {rr.VideoCodec.H264.value}, got {df.select(codec_column).limit(1)}")
# endregion: check_codec

# region: decode_frame
video_column = "/video_stream:VideoStream:sample"
selected_frame_index = 3  # Pick an arbitrary frame to decode

# Query all samples up to and including the target frame.
# We need to decode from the start (or a keyframe) to reach our target.
selected_time = times[selected_frame_index]
video_df = df.filter(col("log_time") <= selected_time).select("log_time", video_column)
pa_table = pa.table(video_df)

# Concatenate samples into a byte buffer
samples = pa_table[video_column].to_numpy()
sample_times = pa_table["log_time"].to_numpy()
sample_bytes = b""
for sample in samples:
    sample_bytes += sample[0].tobytes()
data_buffer = BytesIO(sample_bytes)

# Decode using PyAV
container = av.open(data_buffer, format="h264", mode="r")
video_stream: av.video.stream.VideoStream = container.streams.video[0]
start_time = sample_times[0]

# Decode all frames up to our target, keeping only the last one
frame = None
for packet, time in zip(container.demux(video_stream), sample_times, strict=False):
    packet.time_base = Fraction(1, 1_000_000_000)  # Timestamps in nanoseconds
    packet.pts = int(time - start_time)
    packet.dts = packet.pts  # No B-frames, so dts == pts
    for decoded_frame in packet.decode():
        frame = decoded_frame

if not isinstance(frame, av.VideoFrame):
    raise RuntimeError("Failed to decode frame.")
image = np.asarray(frame.to_image())
print(f"Decoded frame shape: {image.shape}")
# endregion: decode_frame

# region: export_mp4
# Query all video samples
video_df = df.select("log_time", "/video_stream:VideoStream:sample")
pa_table = pa.table(video_df)
all_times = pa_table["log_time"]
all_samples = pa_table["/video_stream:VideoStream:sample"]

# Concatenate samples into a single byte buffer
sample_bytes = np.concatenate([sample[0] for sample in all_samples.to_numpy()]).tobytes()
sample_bytes_io = BytesIO(sample_bytes)

# Setup input container (H.264 Annex B stream)
input_container = av.open(sample_bytes_io, mode="r", format="h264")
input_stream = input_container.streams.video[0]

# Setup output container (MP4)
output_path = "/tmp/output.mp4"
output_container = av.open(output_path, mode="w")
output_stream = output_container.add_stream_from_template(input_stream)

# Remux packets with correct timestamps
start_time = all_times.chunk(0)[0]
for packet, time in zip(input_container.demux(input_stream), all_times, strict=False):
    packet.time_base = Fraction(1, 1_000_000_000)
    packet.pts = int(time.value - start_time.value)
    packet.dts = packet.pts
    packet.stream = output_stream
    output_container.mux(packet)

input_container.close()
output_container.close()
print(f"Exported video to {output_path}")
# endregion: export_mp4
