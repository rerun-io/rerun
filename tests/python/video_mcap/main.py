# TODO(#7484, #9949): Build this out into a proper example.
from __future__ import annotations

import sys
import time

import rerun as rr
from CompressedVideo_pb2 import CompressedVideo
from mcap.reader import make_reader

mcap_path = sys.argv[1]

rr.init("rerun_example_mcap_video", spawn=True)
rr.set_time("time", timestamp=0)


with open(mcap_path, "rb") as f:
    reader = make_reader(f)
    for schema, _channel, message in reader.iter_messages():
        if schema.name == "foxglove.CompressedVideo":
            video_msg = CompressedVideo()
            video_msg.ParseFromString(message.data)
            print(f"Timestamp: {video_msg.timestamp}")
            print(f"Frame ID: {video_msg.frame_id}")
            print(f"Format: {video_msg.format}")
            print(f"Data size: {len(video_msg.data)} bytes")

            print(f"First 16 bytes: {' '.join(f'{b:02x}' for b in video_msg.data[:16])}")

            rr.set_time("time", timestamp=video_msg.timestamp.seconds + video_msg.timestamp.nanos / 1_000_000_000.0)
            rr.log("video_stream", rr.VideoStream(sample=video_msg.data, codec=rr.components.VideoCodec.H264))

            # Slowing down for debugging in-viewer chunk compaction.
            time.sleep(0.005)
