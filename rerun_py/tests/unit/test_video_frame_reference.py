from __future__ import annotations

import numpy as np
import pytest
import rerun as rr


def test_video_frame_reference() -> None:
    rr.set_strict_mode(True)

    # Too many args:
    with pytest.raises(ValueError):
        rr.VideoFrameReference(timestamp=rr.components.VideoTimestamp(seconds=12.3), seconds=12.3, nanoseconds=123)
    with pytest.raises(ValueError):
        rr.VideoFrameReference(seconds=12.3, nanoseconds=123)
    with pytest.raises(ValueError):
        rr.VideoFrameReference(timestamp=rr.components.VideoTimestamp(seconds=12.3), nanoseconds=123)
    with pytest.raises(ValueError):
        rr.VideoFrameReference(seconds=12.3, nanoseconds=123)

    # No args:
    with pytest.raises(ValueError):
        rr.VideoFrameReference()

    # Correct usages:
    assert rr.VideoFrameReference(seconds=12.3).timestamp == rr.components.VideoTimestampBatch(
        rr.components.VideoTimestamp(seconds=12.3),
    )
    assert rr.VideoFrameReference(nanoseconds=123).timestamp == rr.components.VideoTimestampBatch(
        rr.components.VideoTimestamp(nanoseconds=123),
    )
    assert rr.VideoFrameReference(
        timestamp=rr.components.VideoTimestamp(nanoseconds=123),
    ).timestamp == rr.components.VideoTimestampBatch(rr.components.VideoTimestamp(nanoseconds=123))


def test_video_timestamp_batch_rounds_to_nearest_nano() -> None:
    # `1.001 * 1e9` is `1000999999.9999999`: truncating it would drop a nanosecond.
    assert rr.components.VideoTimestamp.seconds([1.001]) == rr.components.VideoTimestampBatch(
        rr.components.VideoTimestamp(nanoseconds=1_001_000_000),
    )
    assert rr.components.VideoTimestamp.milliseconds([1.001]) == rr.components.VideoTimestampBatch(
        rr.components.VideoTimestamp(nanoseconds=1_001_000),
    )


def test_video_timestamp_batch_converts_float32_exactly() -> None:
    # `np.float32(1.001)` is exactly 1.00100004673… s; scaling it in float32 lands on 1001000064.
    seconds = np.array([1.001], dtype=np.float32)
    assert rr.components.VideoTimestamp.seconds(seconds) == rr.components.VideoTimestampBatch(
        rr.components.VideoTimestamp(nanoseconds=1_001_000_047),
    )
    milliseconds = np.array([1001.0], dtype=np.float32)
    assert rr.components.VideoTimestamp.milliseconds(milliseconds) == rr.components.VideoTimestampBatch(
        rr.components.VideoTimestamp(nanoseconds=1_001_000_000),
    )


def test_video_timestamp_scalar_matches_batch() -> None:
    for seconds in [1.001, -1.001]:
        assert rr.components.VideoTimestampBatch(
            rr.components.VideoTimestamp(seconds=seconds)
        ) == rr.components.VideoTimestamp.seconds([seconds])


def test_video_timestamp_scalar_converts_float32_exactly() -> None:
    timestamp = rr.components.VideoTimestamp(seconds=np.float32(1.001))  # type: ignore[arg-type]
    assert timestamp.timestamp_ns == 1_001_000_047
