# TODO(#7484): Build this out into a proper example.
from __future__ import annotations

import platform
import subprocess
import time

import rerun as rr


def capture_webcam_h264() -> None:
    """Capture webcam and stream H.264 encoded video to Rerun with proper timing."""

    # Initialize Rerun
    rr.init("rerun_example_webcam_h264_stream", spawn=True)

    # Target FPS for timing calculations
    target_fps = 30
    frame_duration = 1.0 / target_fps
    frame_time = 0.0

    camera_input = []

    if platform.system() == "Darwin":
        # fmt: off
        camera_input = [
            "-f", "avfoundation",
            # `avfoundation` fails if the framerate is not set.
            "-framerate", str(target_fps),
            # Device 0 for video, nothing for audio
            "-i", "0:none"
        ]
        # fmt: on
    elif platform.system() == "Windows":
        # On windows we *have* to know the device name.
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

        camera_input = ["-f", "dshow", "-framerate", str(target_fps), "-i", f"video={video_device}"]
    else:
        # TODO(#7484): Untested
        camera_input = ["-f", "v4l2", "-i", "0"]

    # FFmpeg command to capture from webcam and output H.264 stream
    # fmt: off
    ffmpeg_cmd = ["ffmpeg"] + camera_input + [
        # Setup encoding.
        "-c:v", "libx264",  # H.264 codec
        "-profile:v", "baseline",  # Baseline profile to avoid b-frames.
        "-pix_fmt", "yuv420p",  # Ensure pixel format compatible with baseline profile.
        "-preset", "veryfast",  # Fast encoding (there's also `superfast` and `ultrafast` for even less latency )
        "-tune", "zerolatency",  # Low latency
        #'-refs', '1',  # Force single reference frame
        #'-bf', '0',  # No B-frames (alternative way to forcing baseline profile)
        # GOP (Group of Pictures) size - smaller value for lower latency.
        # TODO(#7484): This is too aggressive, but needed until we handle gop extending in the viewer better.
        "-g", "4",

        # Setup output.
        "-f", "h264",  # Output format
        "-r", str(target_fps),  # Important, otherwise the output will have a much higher framerate than the input. Practically causing the stream to break down.
        "-",  # Output to stdout
    ]
    # fmt: on

    print("ffmpeg command: " + " ".join(ffmpeg_cmd))

    # Start FFmpeg process
    process = subprocess.Popen(ffmpeg_cmd, stdout=subprocess.PIPE, bufsize=0)

    print(f"Starting webcam capture with H.264 encoding at {target_fps} FPS...")

    try:
        # Buffer to accumulate H.264 data
        buffer = b""
        current_frame_data = b""

        def get_nal_unit_type(nal_data: bytes) -> int | None:
            """Extract NAL unit type from the first byte after start code."""
            if len(nal_data) < 5:  # start code (4 bytes) + at least 1 byte
                return None
            return nal_data[4] & 0x1F  # Lower 5 bits contain the NAL unit type

        def is_frame_chunk_start(nal_type: int) -> bool:
            """Check if this NAL unit type indicates the start of a new frame."""
            # NAL unit types that start a new Access Unit (frame):
            # 1: Non-IDR picture slice
            # 7: SPS (Sequence Parameter Set)
            # ..
            # ffmpeg outputs SPS before each key-frame followed by the actual keyframe.
            # -> if we hit either a non-keyframe or an SPS, we're at the start of a new frame chunk.
            return nal_type in [1, 7]

        while process.poll() is None:
            # Read data from FFmpeg stdout
            if process.stdout is None:
                break
            chunk = process.stdout.read(4096)
            if not chunk:
                break

            buffer += chunk

            # Look for H.264 NAL unit start codes
            while b"\x00\x00\x00\x01" in buffer:
                start_idx = buffer.find(b"\x00\x00\x00\x01")
                next_start = buffer.find(b"\x00\x00\x00\x01", start_idx + 4)

                if next_start == -1:
                    # Incomplete NAL unit, wait for more data
                    break

                # Extract one NAL unit
                nal_data = buffer[start_idx:next_start]
                buffer = buffer[next_start:]

                nal_type = get_nal_unit_type(nal_data)
                if nal_type is None:
                    continue

                # Check if this NAL unit starts a new frame
                if is_frame_chunk_start(nal_type) and current_frame_data:
                    # We have a complete frame, send it to Rerun
                    frame_time += frame_duration
                    rr.set_time("video_time", duration=frame_time)
                    rr.log(
                        "video_stream", rr.VideoStream(sample=current_frame_data, codec=rr.components.VideoCodec.H264)
                    )

                    # Start new frame
                    current_frame_data = nal_data
                else:
                    # Accumulate NAL units for current frame
                    current_frame_data += nal_data

            # Small delay to prevent busy looping too tightly
            time.sleep(0.001)

    except KeyboardInterrupt:
        print("\nStopping capture...")
    except Exception as e:
        print(f"Error: {e}")
    finally:
        # Clean up
        process.terminate()
        process.wait()


if __name__ == "__main__":
    capture_webcam_h264()
