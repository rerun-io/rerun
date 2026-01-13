"""Video decoding and processing utilities."""

from __future__ import annotations

from fractions import Fraction
from io import BytesIO
from typing import TYPE_CHECKING

import av
import numpy as np

from .utils import normalize_times, unwrap_singleton

if TYPE_CHECKING:
    import pyarrow as pa
    import rerun as rr

    from .types import ImageSpec


def extract_video_samples(
    table: pa.Table, sample_column: str, keyframe_column: str, time_column: str
) -> tuple[list[bytes], np.ndarray, np.ndarray]:
    """
    Extract video samples, keyframe flags, and timestamps from a table.

    Args:
        table: PyArrow table containing video data
        sample_column: Name of the column containing video samples
        keyframe_column: Name of the column containing keyframe flags
        time_column: Name of the column containing timestamps

    Returns:
        Tuple of (samples, times_ns, keyframes) where:
        - samples: List of video sample bytes
        - times_ns: Normalized timestamps in nanoseconds
        - keyframes: Boolean array indicating keyframes

    Raises:
        ValueError: If no video samples are available

    """
    samples_raw = table[sample_column].to_pylist()
    keyframes_raw = (
        table[keyframe_column].to_pylist() if keyframe_column in table.column_names else [None] * len(samples_raw)
    )
    times_raw = table[time_column].to_pylist()
    samples: list[bytes] = []
    keyframes: list[bool] = []
    times: list[object] = []
    for sample, keyframe, timestamp in zip(samples_raw, keyframes_raw, times_raw, strict=False):
        sample = unwrap_singleton(sample)
        if sample is None:
            continue
        if isinstance(sample, np.ndarray):
            sample_bytes = sample.tobytes()
        else:
            sample_bytes = bytes(sample)
        samples.append(sample_bytes)
        keyframes.append(bool(unwrap_singleton(keyframe)) if keyframe is not None else False)
        times.append(timestamp)
    if not samples:
        raise ValueError("No video samples available for decoding.")
    return samples, normalize_times(times), np.asarray(keyframes, dtype=bool)


def decode_video_frame(
    samples: list[bytes],
    times_ns: np.ndarray,
    keyframes: np.ndarray,
    target_time_ns: int,
    video_format: str,
) -> np.ndarray:
    """
    Decode a single video frame at the target timestamp.

    Args:
        samples: List of video sample bytes
        times_ns: Timestamps in nanoseconds for each sample
        keyframes: Boolean array indicating keyframes
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

    keyframe_indices = np.where(keyframes)[0]
    if keyframe_indices.size == 0:
        keyframe_idx = 0
    else:
        kf_pos = np.searchsorted(keyframe_indices, idx, side="right") - 1
        keyframe_idx = int(keyframe_indices[max(kf_pos, 0)])

    sample_bytes = b"".join(samples[keyframe_idx : idx + 1])
    data_buffer = BytesIO(sample_bytes)
    container = av.open(data_buffer, format=video_format, mode="r")
    video_stream = container.streams.video[0]
    start_time = times_ns[keyframe_idx]
    latest_frame = None
    packet_times = times_ns[keyframe_idx : idx + 1]
    for packet, time_ns in zip(container.demux(video_stream), packet_times, strict=False):
        packet.time_base = Fraction(1, 1_000_000_000)
        packet.pts = int(time_ns - start_time)
        packet.dts = packet.pts
        for frame in packet.decode():
            latest_frame = frame
    if latest_frame is None:
        raise ValueError("Failed to decode video frame for target time.")
    return np.asarray(latest_frame.to_image())


def infer_video_shape(
    dataset: rr.catalog.DatasetEntry,
    segment_id: str,
    index_column: str,
    spec: ImageSpec,
    video_format: str,
) -> tuple[int, int, int]:
    """
    Infer video frame shape by decoding the first frame.

    Args:
        dataset: Rerun catalog dataset entry
        segment_id: ID of the segment to read from
        index_column: Name of the index/timeline column
        spec: Image specification for the video stream
        video_format: Video codec format

    Returns:
        Tuple of (height, width, channels)

    Raises:
        ValueError: If no video samples are found in the segment

    """
    import pyarrow as pa

    view = dataset.filter_segments(segment_id).filter_contents(spec.path)
    sample_column = f"{spec.path}:VideoStream:sample"
    keyframe_column = f"{spec.path}:is_keyframe"
    df = view.reader(index=index_column).select(index_column, sample_column)
    table = pa.table(df)

    # Check if the table has any rows - if not, the segment might not have video data for this index
    if table.num_rows == 0:
        raise ValueError(
            f"No video data found in segment '{segment_id}' for path '{spec.path}' "
            f"using index '{index_column}'. The segment may not contain video data "
            f"on this timeline, or the video data may use a different index."
        )

    samples, times_ns, keyframes = extract_video_samples(table, sample_column, keyframe_column, index_column)
    target_time_ns = int(times_ns[0])
    decoded = decode_video_frame(samples, times_ns, keyframes, target_time_ns, video_format)
    return decoded.shape
