"""Regression tests for capturing RGB frames directly into a window bank."""

from __future__ import annotations

from typing import TYPE_CHECKING

import av
import numpy as np
import pytest
import torch
from rerun.experimental.dataloader import DecodeRequest, VideoFrameDecoder

from .test_dataloader_decoder_helpers import _encode_h264, _sample_batch

if TYPE_CHECKING:
    from collections.abc import Callable

    from rerun.experimental.dataloader.decoders._base import DecodedValue


def _requests(count: int) -> list[DecodeRequest]:
    return [
        DecodeRequest(
            sample_position=position,
            segment_id="seg",
            index_value=position + 1,
            decode_row_indices=tuple(range(position + 2)),
            output_row_indices=(position, position + 1),
            starts_at_keyframe=True,
        )
        for position in range(count - 1)
    ]


@pytest.mark.parametrize("scenario", ["mixed_resolution", "missing_frame"])
def test_rgb_bank_handles_incompatible_or_missing_frames(scenario: str, monkeypatch: pytest.MonkeyPatch) -> None:
    batch = _sample_batch(_encode_h264(num_frames=4, gop=4), is_windowed=True)
    frames = [
        av.VideoFrame.from_ndarray(
            np.full((8, 16 if scenario == "mixed_resolution" and index >= 2 else 8, 3), index * 20, dtype=np.uint8),
            format="rgb24",
        )
        for index in range(4)
    ]
    available = {0, 1, 3} if scenario == "missing_frame" else set(range(4))
    decoder = VideoFrameDecoder(codec="h264", window_storage="view")

    def feed(
        _segment: str,
        _samples: list[bytes],
        wanted: list[int],
        *,
        capture_frame: Callable[[int, av.VideoFrame], DecodedValue] | None = None,
    ) -> dict[int, DecodedValue]:
        assert capture_frame is not None
        return {position: capture_frame(position, frames[position]) for position in wanted if position in available}

    monkeypatch.setattr(decoder, "_feed_run", feed)
    actual = decoder.decode(batch, _requests(4))

    for position, value in enumerate(actual):
        if not {position, position + 1} <= available or frames[position].width != frames[position + 1].width:
            assert value is None
        else:
            expected = torch.stack([
                torch.from_numpy(frame.to_ndarray()).permute(2, 0, 1) for frame in frames[position : position + 2]
            ])
            assert value is not None
            torch.testing.assert_close(value, expected, rtol=0, atol=0)
