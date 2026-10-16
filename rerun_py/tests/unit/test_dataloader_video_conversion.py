"""Regression tests for reusing RGB conversion contexts."""

from __future__ import annotations

import threading
from concurrent.futures import ThreadPoolExecutor

import av
import numpy as np
import torch
from rerun.experimental.dataloader import VideoFrameDecoder


def _frame(width: int, height: int, pixel_format: str, colorspace: int, color_range: int) -> av.VideoFrame:
    frame = av.VideoFrame(width, height, pixel_format)
    rng = np.random.default_rng(width)
    for plane in frame.planes:
        plane.update(rng.integers(0, 256, size=plane.buffer_size, dtype=np.uint8).tobytes())
    frame.colorspace = colorspace
    frame.color_range = color_range
    return frame


def test_reused_rgb_converter_preserves_pixels_when_format_changes() -> None:
    decoder = VideoFrameDecoder()
    retained = []
    for width, height, pixel_format, colorspace, color_range in [
        (32, 24, "yuv420p", 1, 1),
        (32, 24, "yuv420p", 5, 2),
        (40, 28, "yuvj420p", 5, 2),
        (32, 24, "rgb24", 1, 2),
    ]:
        frame = _frame(width, height, pixel_format, colorspace, color_range)
        expected = torch.from_numpy(frame.to_ndarray(format="rgb24")).permute(2, 0, 1).clone()
        actual = decoder._frame_to_tensor(frame)
        retained.append((actual, expected))
        del frame

    del decoder
    for actual, expected in retained:
        torch.testing.assert_close(actual, expected, rtol=0, atol=0)


def test_rgb_converter_is_reused_per_thread() -> None:
    decoder = VideoFrameDecoder()
    barrier = threading.Barrier(2)

    def convert() -> object:
        frame = _frame(32, 24, "yuv420p", 1, 1)
        barrier.wait(timeout=10)
        decoder._frame_to_tensor(frame)
        assert decoder._rgb_reformatters is not None
        context = decoder._rgb_reformatters.rgb
        decoder._frame_to_tensor(frame)
        assert decoder._rgb_reformatters.rgb is context
        return context

    with ThreadPoolExecutor(max_workers=2) as executor:
        futures = [executor.submit(convert) for _ in range(2)]
        first, second = [future.result() for future in futures]

    assert first is not second
