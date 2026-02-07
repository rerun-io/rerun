"""Video encode images using av and stream them to Rerun."""

import av
import numpy as np
import numpy.typing as npt
import rerun as rr

fps = 30
duration_seconds = 4
width = 480
height = 320
ball_radius = 30
codec = rr.VideoCodec.H265  # rr.VideoCodec.H264

formats = {rr.VideoCodec.H265: "hevc", rr.VideoCodec.H264: "h264"}
encoders = {rr.VideoCodec.H265: "libx265", rr.VideoCodec.H264: "libx264"}


def create_example_video_frame(frame_i: int) -> npt.NDArray[np.uint8]:
    img = np.zeros((height, width, 3), dtype=np.uint8)
    for h in range(height):
        img[h, :] = [0, int(100 * h / height), int(200 * h / height)]  # Blue to purple gradient.

    x_pos = width // 2  # Center horizontally.
    y_pos = height // 2 + 80 * np.sin(2 * np.pi * frame_i / fps)
    y, x = np.ogrid[:height, :width]
    r_sq = (x - x_pos) ** 2 + (y - y_pos) ** 2
    img[r_sq < ball_radius**2] = [255, 200, 0]  # Gold color

    return img


rr.init("rerun_example_video_stream_synthetic")

# Setup encoding pipeline.
av.logging.set_level(av.logging.VERBOSE)
container = av.open("/dev/null", "w", format=formats[codec])  # Use AnnexB H.265 stream.
stream = container.add_stream(encoders[codec], rate=fps)
# Type narrowing
assert isinstance(stream, av.video.stream.VideoStream)
stream.width = width
stream.height = height
# TODO(#10090): Rerun Video Streams don't support b-frames yet.
# Note that b-frames are generally not recommended for low-latency streaming and may make logging more complex.
stream.max_b_frames = 0

# Log codec only once as static data (it naturally never changes). This isn't strictly necessary, but good practice.
rr.log("video_stream", rr.VideoStream(codec=codec), static=True)

# Generate frames and stream them directly to Rerun.
for frame_i in range(fps * duration_seconds):
    img = create_example_video_frame(frame_i)
    frame = av.VideoFrame.from_ndarray(img, format="rgb24")
    for packet in stream.encode(frame):
        if packet.pts is None:
            continue
        rr.set_time("time", duration=float(packet.pts * packet.time_base))
        rr.log("video_stream", rr.VideoStream.from_fields(sample=bytes(packet)))

# Flush stream.
for packet in stream.encode():
    if packet.pts is None:
        continue
    rr.set_time("time", duration=float(packet.pts * packet.time_base))
    rr.log("video_stream", rr.VideoStream.from_fields(sample=bytes(packet)))
