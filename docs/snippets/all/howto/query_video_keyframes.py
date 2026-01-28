"""Query video streams efficiently using keyframe information."""

# ruff: noqa: E402

from __future__ import annotations

import atexit
import pathlib
import shutil
import tempfile


TMP_DIR = pathlib.Path(tempfile.mkdtemp())
atexit.register(lambda: shutil.rmtree(TMP_DIR) if TMP_DIR.exists() else None)


# region: setup
from io import BytesIO
from pathlib import Path
from fractions import Fraction

import av
import numpy as np
import pyarrow as pa
from datafusion import col, functions as F

import rerun as rr

sample_video_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "video_sample"

server = rr.server.Server(datasets={"video_dataset": sample_video_path})
client = server.client()
dataset = client.get_dataset(name="video_dataset")
df = dataset.filter_contents(["/video_stream/**"]).reader(index="log_time")
times = pa.table(df.select("log_time"))["log_time"].to_numpy()
video_column = "/video_stream:VideoStream:sample"
# endregion: setup

# region: add_keyframe_column
# Preprocessing step: Add keyframe information to existing video data as a layer
# This is typically done once to make subsequent queries faster

# Query all video samples from the existing recording
video_samples_df = df.select("log_time", video_column)
video_table = pa.table(video_samples_df)
sample_times = video_table["log_time"].to_numpy()
samples = video_table[video_column].to_numpy()

# Concatenate all samples to analyze keyframes
sample_bytes = b""
for sample in samples:
    sample_bytes += sample[0].tobytes()

# Decode the video to detect keyframes
data_buffer = BytesIO(sample_bytes)
container = av.open(data_buffer, format="h264", mode="r")
video_stream = container.streams.video[0]

# Identify which samples are keyframes
keyframe_times = []
for packet, ts in zip(container.demux(video_stream), sample_times):
    if packet.is_keyframe:
        keyframe_times.append(ts)
container.close()
keyframe_values = [True] * len(keyframe_times)

print(f"Found {len(keyframe_times)} keyframes")

# Save keyframe data as a separate layer
# Get the segment ID to align with the original recording
segment_ids = dataset.segment_ids()
first_segment_id = segment_ids[0]

# Create time column and content using the columnar API
# Make sure the timeline matches the original video stream
timeline = "log_time"
time_column = rr.TimeColumn(timeline=timeline, timestamp=keyframe_times)
content = rr.DynamicArchetype.columns(archetype="KeyframeData", components={"is_keyframe": keyframe_values})

# Write to a new file as a layer
layer_path = TMP_DIR / "keyframe_layer.rrd"
with rr.RecordingStream(
    application_id="keyframes",
    recording_id=first_segment_id,  # Match original recording_id
) as rec:
    rec.save(layer_path)
    rec.send_columns("/video_stream", indexes=[time_column], columns=[*content])

# Register the layer with the dataset
dataset.register(layer_path.as_uri(), layer_name="keyframes")
print(f"Registered keyframe layer at {layer_path}")
# endregion: add_keyframe_column

# region: query_with_keyframes
# Query using keyframe information for efficient random access
# Assume we've already added keyframe information via the preprocessing step above
target_frame_index = 42
target_time = times[target_frame_index]

# Create a reader that includes the keyframe layer data
# The column name follows the pattern: /{entity_path}:{component_name}
keyframe_column = "/video_stream:is_keyframe"
full_df = dataset.filter_contents(["/video_stream/**"]).reader(index="log_time")

# Query to find the most recent keyframe at or before the target time
# Since we only log when is_keyframe=True, any row with this column present is a keyframe
keyframe_slice = full_df.filter((col("log_time") <= target_time) & col(keyframe_column).is_not_null())
closest_keyframe_df = keyframe_slice.aggregate(
    [], [F.last_value(col("log_time"), order_by=[col("log_time")]).alias("latest_keyframe")]
)

keyframe_result = pa.table(closest_keyframe_df)

# Start decoding from the most recent keyframe
start_time = keyframe_result["latest_keyframe"].to_numpy()[0]
start_frame_idx = np.searchsorted(times, start_time)

frames_saved = target_frame_index - start_frame_idx
print(f"Found keyframe at frame {start_frame_idx}, saved decoding {frames_saved} frames")

# Query only the video samples from keyframe to target (much more efficient!)
efficient_video_df = df.filter(col("log_time").between(start_time, target_time)).select("log_time", video_column)

efficient_table = pa.table(efficient_video_df)
frames_to_decode = len(efficient_table)
print(f"Decoding {frames_to_decode} frames (vs {target_frame_index + 1} without keyframe info)")

# Now decode just this smaller range
samples = efficient_table[video_column].to_numpy()
sample_times = efficient_table["log_time"].to_numpy()
sample_bytes = b""
for sample in samples:
    sample_bytes += sample[0].tobytes()

data_buffer = BytesIO(sample_bytes)
container = av.open(data_buffer, format="h264", mode="r")
video_stream = container.streams.video[0]

# Decode to the target frame
frame = None
for packet, time in zip(container.demux(video_stream), sample_times, strict=False):
    packet.time_base = Fraction(1, 1_000_000_000)
    packet.pts = int(time - sample_times[0])
    packet.dts = packet.pts
    for decoded_frame in packet.decode():
        frame = decoded_frame

if isinstance(frame, av.VideoFrame):
    image = np.asarray(frame.to_image())
    print(f"Efficiently decoded frame {target_frame_index} with shape: {image.shape}")
# endregion: query_with_keyframes
