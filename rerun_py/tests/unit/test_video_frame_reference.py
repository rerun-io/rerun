from __future__ import annotations

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
