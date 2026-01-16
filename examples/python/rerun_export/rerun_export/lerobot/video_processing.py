"""Video decoding and processing utilities."""

from __future__ import annotations

from fractions import Fraction
from io import BytesIO
from pathlib import Path
from typing import TYPE_CHECKING

import av
import numpy as np
import pyarrow as pa

from rerun_export.utils import normalize_times, unwrap_singleton

if TYPE_CHECKING:
    import rerun as rr

    from rerun_export.lerobot.types import VideoSpec


def extract_video_samples(table: pa.Table, *, sample_column: str, time_column: str) -> tuple[list[bytes], np.ndarray]:
    """
    Extract video samples and timestamps from a table.

    Args:
        table: PyArrow table containing video data
        sample_column: Name of the column containing video samples
        time_column: Name of the column containing timestamps

    Returns:
        Tuple of (samples, times_ns) where:
        - samples: List of video sample bytes
        - times_ns: Normalized timestamps in nanoseconds

    Raises:
        ValueError: If no video samples are available

    """
    samples_raw = table[sample_column].to_pylist()
    times_raw = table[time_column].to_pylist()
    samples: list[bytes] = []
    times: list[object] = []
    for sample, timestamp in zip(samples_raw, times_raw, strict=False):
        sample = unwrap_singleton(sample)
        if sample is None:
            continue
        if isinstance(sample, np.ndarray):
            sample_bytes = sample.tobytes()
        else:
            sample_bytes = bytes(sample)
        samples.append(sample_bytes)
        times.append(timestamp)
    if not samples:
        raise ValueError("No video samples available for decoding.")
    return samples, normalize_times(times)


def decode_video_frame(
    *,
    samples: list[bytes],
    times_ns: np.ndarray,
    target_time_ns: int,
    video_format: str,
) -> np.ndarray:
    """
    Decode a single video frame at the target timestamp.

    Without keyframe information, decodes from the beginning up to the target frame.

    Args:
        samples: List of video sample bytes
        times_ns: Timestamps in nanoseconds for each sample
        target_time_ns: Target timestamp to decode at
        video_format: Video codec format (e.g., "h264", "hevc")

    Returns:
        Decoded frame as numpy array

    Raises:
        ValueError: If frame decoding fails

    """
    idx = int(np.searchsorted(times_ns, target_time_ns, side="right") - 1)
    if idx < 0:
        idx = 0

    # Without keyframe info, decode from the beginning
    sample_bytes = b"".join(samples[: idx + 1])
    data_buffer = BytesIO(sample_bytes)
    container = av.open(data_buffer, format=video_format, mode="r")
    video_stream = container.streams.video[0]
    start_time = times_ns[0]
    latest_frame = None
    packet_times = times_ns[: idx + 1]
    for packet, time_ns in zip(container.demux(video_stream), packet_times, strict=False):
        packet.time_base = Fraction(1, 1_000_000_000)
        packet.pts = int(time_ns - start_time)
        packet.dts = packet.pts
        for frame in packet.decode():
            latest_frame = frame
    if latest_frame is None:
        raise ValueError("Failed to decode video frame for target time.")
    return np.asarray(latest_frame.to_image())


def can_remux_video(
    times_ns: np.ndarray,
    target_fps: int,
    tolerance: float = 0.05,
) -> tuple[bool, float]:
    """
    Check if video can be remuxed without re-encoding.

    Compares the source frame rate (inferred from timestamps) with target FPS.
    If they match within tolerance, remuxing is possible.

    Args:
        times_ns: Timestamps in nanoseconds for each packet
        target_fps: Target frames per second
        tolerance: Allowed relative difference (default 5%)

    Returns:
        Tuple of (can_remux, source_fps) where:
        - can_remux: True if source and target FPS match within tolerance
        - source_fps: Detected source FPS

    """
    if len(times_ns) < 2:
        return False, 0.0

    # Calculate median frame interval in nanoseconds
    intervals = np.diff(times_ns)
    avg_interval_ns = np.median(intervals)  # Use median to handle outliers

    # Convert to FPS
    source_fps = 1_000_000_000 / avg_interval_ns

    # Check if within tolerance
    fps_diff = abs(source_fps - target_fps) / target_fps
    can_remux = fps_diff <= tolerance

    return can_remux, source_fps


def remux_video_stream(
    samples: list[bytes],
    times_ns: np.ndarray,
    *,
    output_path: str,
    video_format: str,
    width: int | None = None,
    height: int | None = None,
    target_fps: int | None = None,
) -> None:
    """
    Remux compressed video packets directly to output file without decode/encode.

    This is 100-1000x faster than decoding to frames and re-encoding. Use this when:
    - The source video FPS matches target FPS (or target_fps is None)
    - No frame transformations are needed

    Args:
        samples: List of compressed video packet bytes from RRD
        times_ns: Timestamps in nanoseconds for each packet
        output_path: Path to write output video file
        video_format: Source codec format (e.g., "h264", "hevc")
        width: Video frame width (auto-detected if None)
        height: Video frame height (auto-detected if None)
        target_fps: Target FPS (if None, preserves source timing)

    Raises:
        ValueError: If remuxing fails

    """

    Path(output_path).parent.mkdir(parents=True, exist_ok=True)

    # Create input container from concatenated samples
    all_samples = b"".join(samples)
    input_buffer = BytesIO(all_samples)
    input_container = av.open(input_buffer, format=video_format, mode="r")
    input_stream = input_container.streams.video[0]

    # Auto-detect dimensions if not provided
    if width is None or width <= 0:
        width = input_stream.width
    if height is None or height <= 0:
        height = input_stream.height
    if width in (None, 0) or height in (None, 0):
        raise ValueError(
            "Video dimensions are missing from the encoded stream. "
            "Provide width/height when calling remux_video_stream (e.g. from inferred feature shapes) "
            "or ensure SPS/PPS metadata is present."
        )

    # Create output container (MP4 format)
    output_container = av.open(output_path, mode="w", format="mp4")

    # Add stream from template to preserve codec without re-encoding.
    # Some PyAV versions don't support the template kwarg; fall back to codec name.
    try:
        output_stream = output_container.add_stream(template=input_stream)
    except TypeError:
        output_stream = output_container.add_stream(input_stream.codec_context.name)
    if not output_stream.codec_context.extradata and input_stream.codec_context.extradata:
        output_stream.codec_context.extradata = input_stream.codec_context.extradata
    output_stream.width = width
    output_stream.height = height
    output_stream.codec_context.width = width
    output_stream.codec_context.height = height

    if target_fps is not None:
        output_stream.rate = target_fps

    # Calculate time base from original timestamps
    time_base = Fraction(1, 1_000_000_000)  # nanosecond precision
    output_stream.time_base = time_base

    # Remux packets with proper timestamps
    packet_idx = 0
    for packet in input_container.demux(input_stream):
        if packet_idx >= len(times_ns):
            break

        # Set timestamps from RRD data
        packet.time_base = time_base
        packet.pts = int(times_ns[packet_idx])
        packet.dts = packet.pts
        packet.stream = output_stream

        output_container.mux(packet)
        packet_idx += 1

    input_container.close()
    output_container.close()


def infer_video_shape(
    dataset: rr.catalog.DatasetEntry,
    segment_id: str,
    index_column: str,
    spec: VideoSpec,
) -> tuple[int, int, int]:
    """
    Infer video frame shape by decoding the first frame.

    Args:
        dataset: Rerun catalog dataset entry
        segment_id: ID of the segment to read from
        index_column: Name of the index/timeline column
        spec: Image specification for the video stream
    Returns:
        Tuple of (height, width, channels)

    Raises:
        ValueError: If no video samples are found in the segment

    """

    view = dataset.filter_segments(segment_id).filter_contents(spec.path)
    sample_column = f"{spec.path}:VideoStream:sample"
    df = view.reader(index=index_column).select(index_column, sample_column)
    table = pa.table(df)

    # Check if the table has any rows - if not, the segment might not have video data for this index
    if table.num_rows == 0:
        raise ValueError(
            f"No video data found in segment '{segment_id}' for path '{spec.path}' "
            f"using index '{index_column}'. The segment may not contain video data "
            "on this timeline, or the video data may use a different index."
        )

    samples, times_ns = extract_video_samples(table, sample_column=sample_column, time_column=index_column)
    target_time_ns = int(times_ns[0])
    decoded = decode_video_frame(
        samples=samples, times_ns=times_ns, target_time_ns=target_time_ns, video_format=spec.video_format
    )
    return decoded.shape
