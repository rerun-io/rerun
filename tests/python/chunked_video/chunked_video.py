"""Logs all videos in a folder to a single entity in order."""

# Order is either alphabetical or - if present - by last filename digits parsed as integer.
#
# Any folder of .mp4 files can be used.
# For Rerun internal users, there are examples assets available at
# https://github.com/rerun-io/internal-test-assets/tree/main/video
#
# Things to look out for:
# * are all chunks arriving, are things crashing
# * is the video playing smooth on video asset transitions
# * does seeking across and within video assets work
# * does memory usage induced by mp4 parsing stay low over time and doesn't accumluate (TODO(#7481): it should be smaller to begin with)
from __future__ import annotations

import re
import sys
from pathlib import Path

import rerun as rr


# Try sorting by last filename digits parsed as integer.
def get_trailing_number(filename: Path) -> int:
    match = re.search(r"\d+$", filename.stem)
    return int(match.group()) if match else 0


def main() -> None:
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <path_to_folder>")
        sys.exit(1)

    rr.init("rerun_example_chunked_video", spawn=True)

    video_folder = Path(sys.argv[1])
    video_files = [file for file in video_folder.iterdir() if file.suffix == ".mp4"]

    video_files.sort(key=get_trailing_number)

    last_time_ns = 0
    for file in video_files:
        print(f"Logging video {file}, start time {last_time_ns}ns")

        rr.set_time("video_time", duration=1e-9 * last_time_ns)

        video_asset = rr.AssetVideo(path=file)
        rr.log("video", video_asset)

        frame_timestamps_ns = video_asset.read_frame_timestamps_ns()
        rr.send_columns(
            "video",
            # Note timeline values don't have to be the same as the video timestamps.
            times=[rr.TimeColumn("video_time", duration=1e-9 * (frame_timestamps_ns + last_time_ns))],
            columns=rr.VideoFrameReference.columns_nanoseconds(frame_timestamps_ns),
        )
        last_time_ns += frame_timestamps_ns[-1]


if __name__ == "__main__":
    main()
