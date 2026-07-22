from __future__ import annotations

import pytest
from rerun.components import VideoCodec
from rerun.experimental.video import (
    detect_gop_start,
    is_annex_b,
    length_prefixed_to_annex_b,
)

ANNEX_B_START_CODE = b"\x00\x00\x00\x01"

# H.264 SPS and IDR NAL units, matching the `re_video` GOP detection tests.
SPS_NALU = bytes([
    0x67, 0x64, 0x00, 0x0A, 0xAC, 0x72, 0x84, 0x44, 0x26, 0x84, 0x00, 0x00, 0x03,
    0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xCA, 0x3C, 0x48, 0x96, 0x11, 0x80,
])  # fmt: skip
IDR_NALU = bytes([
    0x65, 0x88, 0x84, 0x21, 0x43, 0x02, 0x4C, 0x82, 0x54, 0x2B, 0x8F, 0x2C, 0x8C,
    0x54, 0x4A, 0x92, 0x54, 0x2B, 0x8F, 0x2C, 0x8C, 0x54, 0x4A, 0x92,
])  # fmt: skip

H264_KEYFRAME_ANNEX_B = ANNEX_B_START_CODE + SPS_NALU + ANNEX_B_START_CODE + IDR_NALU


def _length_prefixed(nalus: list[bytes], length_prefix_size: int = 4) -> bytes:
    return b"".join(len(nalu).to_bytes(length_prefix_size, "big") + nalu for nalu in nalus)


@pytest.mark.parametrize(
    ("data", "expected"),
    [
        (b"\x00\x00\x00\x01\xab\xcd", True),  # 4-byte start code
        (b"\x00\x00\x01\xab\xcd", True),  # 3-byte short start code
        (b"\x00\x00\x00\x01", True),  # exactly the start code
        (b"\x00\x00\x01", True),  # exactly the short start code
        (b"\x00\x00\x02\xab", False),
        (b"\xab\xcd\xef\x01", False),
        (b"", False),
        (b"\x00", False),
        (b"\x00\x00", False),
    ],
)
def test_is_annex_b(data: bytes, expected: bool) -> None:
    assert is_annex_b(data) is expected


def test_detect_gop_start_h264_keyframe() -> None:
    assert detect_gop_start(H264_KEYFRAME_ANNEX_B, VideoCodec.H264)


def test_detect_gop_start_h264_non_keyframe() -> None:
    assert not detect_gop_start(ANNEX_B_START_CODE + SPS_NALU, VideoCodec.H264)
    assert not detect_gop_start(bytes(range(1, 11)), VideoCodec.H264)


def test_detect_gop_start_h264_broken_sps() -> None:
    broken_sps = bytes([0x67, 0x00]) + SPS_NALU[2:]
    with pytest.raises(ValueError, match="Failed reading SPS"):
        detect_gop_start(ANNEX_B_START_CODE + broken_sps + ANNEX_B_START_CODE + IDR_NALU, VideoCodec.H264)


def test_length_prefixed_to_annex_b() -> None:
    length_prefixed = _length_prefixed([SPS_NALU, IDR_NALU])
    assert length_prefixed_to_annex_b(length_prefixed) == H264_KEYFRAME_ANNEX_B


def test_length_prefixed_to_annex_b_short_prefix() -> None:
    length_prefixed = _length_prefixed([SPS_NALU, IDR_NALU], length_prefix_size=2)
    assert length_prefixed_to_annex_b(length_prefixed, length_prefix_size=2) == H264_KEYFRAME_ANNEX_B


def test_length_prefixed_to_annex_b_truncated() -> None:
    length_prefixed = _length_prefixed([SPS_NALU, IDR_NALU])
    with pytest.raises(ValueError, match="incomplete NAL unit"):
        length_prefixed_to_annex_b(length_prefixed[:-1])


def test_length_prefixed_round_trip_detects_gop() -> None:
    annex_b = length_prefixed_to_annex_b(_length_prefixed([SPS_NALU, IDR_NALU]))
    assert detect_gop_start(annex_b, VideoCodec.H264)
