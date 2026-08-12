"""Utilities for working with encoded video sample streams."""

from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import (
    video_detect_gop_start,
    video_length_prefixed_to_annex_b as length_prefixed_to_annex_b,
)

if TYPE_CHECKING:
    from ..components import VideoCodec

__all__ = [
    "detect_gop_start",
    "is_annex_b",
    "length_prefixed_to_annex_b",
]


def is_annex_b(sample: bytes) -> bool:
    """Whether the sample starts with an Annex B start code (`00 00 01` or `00 00 00 01`)."""
    return sample.startswith((b"\x00\x00\x00\x01", b"\x00\x00\x01"))


def detect_gop_start(sample: bytes, codec: VideoCodec) -> bool:
    """
    Detect whether a video sample starts a group of pictures, i.e. is a keyframe.

    H.264/H.265 samples must be in Annex B format.
    """
    return video_detect_gop_start(sample, codec.value)
