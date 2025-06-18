"""Video encode live camera stream using av and stream them to Rerun."""

import platform
import subprocess

import av
import rerun as rr


av.logging.set_level(av.logging.VERBOSE)

rr.init("rerun_example_video_stream_camera", spawn=True)

# Setup platform dependent camera input.
fps = 30
if platform.system() == "Darwin":
    input_container = av.open(
        "0",
        format="avfoundation",
        container_options={"framerate": f"{fps}"},  # `avfoundation` fails if the framerate is not set.
    )
elif platform.system() == "Windows":
    # On windows we *have* to know the device name. Query ffmpeg for it.
    list_devices_cmd = ["ffmpeg", "-f", "dshow", "-list_devices", "true", "-i", "dummy", "-hide_banner"]
    devices_output = subprocess.check_output(list_devices_cmd, stderr=subprocess.STDOUT).decode()

    # Extract video device names
    video_devices = []
    for line in devices_output.split("\n"):
        if '"' in line and "Alternative name" not in line:
            device_name = line.split('"')[1]
            if device_name not in video_devices:
                video_devices.append(device_name)
    if not video_devices:
        raise RuntimeError("No video devices found")

    # Use first available video device
    video_device = video_devices[0]
    print(f"Using video device: {video_device}")

    input_container = av.open(f"video={video_device}", format="dshow")
else:
    input_container = av.open("0", format="v4l2")

# Setup output.
output_container = av.open("/dev/null", "w", format="h264")  # Use AnnexB H.264 stream.
output_stream = output_container.add_stream("libx264", rate=fps)
output_stream.width = input_container.streams.video[0].width
output_stream.height = input_container.streams.video[0].height
# Tune for low latency.
output_stream.options = {"preset": "veryfast", "tune": "zerolatency"}
output_stream.max_b_frames = 0

# Log codec only once as static data (it naturally never changes). This isn't strictly necessary, but good practice.
rr.log("video_stream", rr.VideoStream(codec=rr.VideoCodec.H264), static=True)

# Stream camera images continuously to Rerun.
try:
    while True:
        try:
            for frame in input_container.decode(video=0):
                for packet in output_stream.encode(frame):
                    if packet.pts is None:
                        continue
                    rr.set_time("video_stream", duration=float(packet.pts * packet.time_base))
                    rr.log("video_stream", rr.VideoStream.from_fields(sample=bytes(packet)))
        except av.BlockingIOError:
            pass
except KeyboardInterrupt:
    print("Recording stopped by user")
