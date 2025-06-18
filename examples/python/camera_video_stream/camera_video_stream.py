#!/usr/bin/env python3
from __future__ import annotations

import argparse
import platform
import subprocess

import av
import rerun as rr


def setup_camera_input(video_device: str | None = None) -> av.container.InputContainer:
    """
    Setup the camera input container.

    Uses first available video device. Exact behavior is platform dependent.
    """
    if platform.system() == "Darwin":
        if video_device is None:
            video_device = "0"

        return av.open(
            video_device,
            format="avfoundation",
            container_options={"framerate": "30"},  # `avfoundation` fails if the framerate is not set.
        )
    elif platform.system() == "Windows":
        if video_device is None:
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

        return av.open(f"video={video_device}", format="dshow")
    else:
        if video_device is None:
            video_device = "0"

        return av.open(video_device, format="v4l2")


def setup_output_stream(width: int, height: int) -> av.video.VideoStream:
    """Setup the output stream which encodes the video stream to H.264."""

    output_container = av.open("/dev/null", "w", format="h264")  # Use AnnexB H.264 stream.
    output_stream = output_container.add_stream("libx264")
    output_stream.width = width
    output_stream.height = height

    # Configure for low latency.
    output_stream.codec_context.options = {
        "tune": "zerolatency",
        "preset": "veryfast",
    }
    output_stream.max_b_frames = 0  # Avoid b-frames for lower latency.

    return output_stream


def stream_video_to_rerun(input: av.container.InputContainer, output: av.video.VideoStream) -> None:
    """Streams the video continuously to Rerun."""

    # Log codec only once as static data (it naturally never changes). This isn't strictly necessary, but good practice.
    rr.log("video_stream", rr.VideoStream(codec=rr.VideoCodec.H264), static=True)

    while True:
        try:
            for frame in input.decode(video=0):
                # By default all the frames that come from the camera are marked as I-frames.
                # If we pass this just on as-is, then we get an encoded video stream that
                # just consists entirely of I-frames, thus having very poor compression!
                # Instead, we want the encoder to decide when to use P & I frames.
                frame.pict_type = av.video.frame.PictureType.NONE

                for packet in output.encode(frame):
                    if packet.pts is None:
                        continue
                    rr.set_time("video_stream", duration=float(packet.pts * packet.time_base))
                    rr.log("video_stream", rr.VideoStream.from_fields(sample=bytes(packet)))
        except av.BlockingIOError:
            pass


def main() -> None:
    parser = argparse.ArgumentParser(description="Streams compressed video from camera to Rerun.")
    parser.add_argument(
        "--video-device",
        type=str,
        help="Video device to use. If not provided, the first available video device will be used.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_video_stream_camera")

    av.logging.set_level(av.logging.VERBOSE)

    input_container = setup_camera_input(args.video_device)
    output_stream = setup_output_stream(input_container.streams.video[0].width, input_container.streams.video[0].height)

    try:
        stream_video_to_rerun(input_container, output_stream)
    except KeyboardInterrupt:
        print("Recording stopped by user")

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
