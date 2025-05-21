import mcap
from mcap.reader import make_reader
import rerun as rr
import sys
from CompressedVideo_pb2 import CompressedVideo


mcap_path = sys.argv[1]

rr.init("rerun_example_mcap_video", spawn=True)
rr.set_time("time", sequence=0)


t = 0

with open(mcap_path, "rb") as f:
    reader = make_reader(f)
    for schema, channel, message in reader.iter_messages():
        if schema.name == "foxglove.CompressedVideo":
            video_msg = CompressedVideo()
            video_msg.ParseFromString(message.data)
            print(f"Timestamp: {video_msg.timestamp}")
            print(f"Frame ID: {video_msg.frame_id}")
            print(f"Format: {video_msg.format}")
            print(f"Data size: {len(video_msg.data)} bytes")

            t += 1
            rr.set_time("time", sequence=t)
            rr.log("video_stream", rr.VideoStream(video_msg.data))



        #

# for i, frame in enumerate(frames):
#     rr.set_time("frame", sequence=i)
#     rr.log("raw_image", rr.Image(frame))


