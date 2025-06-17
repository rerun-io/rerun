"""Video encode images using av and stream them to Rerun."""

from fractions import Fraction

import av
import numpy as np
import rerun as rr


def log_video_sample(packet: av.Packet) -> None:
    """Log a H.264 video sample to Rerun."""
    rr.set_time("video_stream", duration=float(packet.pts * packet.time_base))
    rr.log("video_stream", rr.VideoStream(sample=bytes(packet), codec="h264"))


fps = 30
duration_seconds = 4
width = 480
height = 320
ball_radius = 30

rr.init("rerun_example_video_stream", spawn=True)

# Setup encoding pipeline.
av.logging.set_level(av.logging.VERBOSE)
container = av.open("/dev/null", "w", format="h264")  # Use AnnexB H.264 stream.
stream = container.add_stream("libx264", rate=fps)
stream.width = width
stream.height = height
stream.rate = Fraction(fps, 1)
# TODO(#10090): Rerun Video Streams don't support b-frames yet.
# Note however, that b-frames are generally not recommended for low-latency streaming
# and may make the logging process a lot more complex.
stream.max_b_frames = 0

for frame_i in range(fps * duration_seconds):
    # Add gradient background.
    img = np.zeros((height, width, 3), dtype=np.uint8)
    for y in range(height):
        img[y, :] = [0, int(100 * y / height), int(200 * y / height)]  # Blue to purple gradient.

    # Calculate ball position using sine wave for bouncing effect.
    x_pos = width // 2  # Center horizontally.
    y_pos = height // 2 + 80 * np.sin(2 * np.pi * frame_i / fps)
    y, x = np.ogrid[:height, :width]
    r_sq = (x - x_pos) ** 2 + (y - y_pos) ** 2
    img[r_sq < ball_radius**2] = [255, 200, 0]  # Gold color

    # Encode frame and log to rerun.
    frame = av.VideoFrame.from_ndarray(img, format="rgb24")
    for packet in stream.encode(frame):
        log_video_sample(packet)

# Flush stream.
for packet in stream.encode():
    log_video_sample(packet)
