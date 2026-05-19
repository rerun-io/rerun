"""Tests for the low-level helpers in `rerun.experimental.dataloader._decoders`."""

from __future__ import annotations

import numpy as np
import pyarrow as pa
import pytest
from rerun.experimental.dataloader._decoders import (
    _avcc_to_annex_b,
    _flatten_blob,
    _is_annex_b,
    _is_av1_keyframe_packet,
    _unwrap_to_numpy,
)


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
    assert _is_annex_b(data) is expected


def _make_obu_header(obu_type: int) -> int:
    """Return a byte whose OBU-type field (bits [3:6]) matches *obu_type*."""
    return (obu_type & 0xF) << 3


@pytest.mark.parametrize(
    ("obu_type", "expected"),
    [
        (1, True),  # OBU_SEQUENCE_HEADER
        (2, True),  # OBU_TEMPORAL_DELIMITER
        (3, False),  # OBU_FRAME_HEADER
        (6, False),  # OBU_FRAME
        (0, False),
        (15, False),
    ],
)
def test_is_av1_keyframe_packet(obu_type: int, expected: bool) -> None:
    sample = bytes([_make_obu_header(obu_type), 0x00, 0x00])
    assert _is_av1_keyframe_packet(sample) is expected


def test_is_av1_keyframe_packet_empty() -> None:
    assert _is_av1_keyframe_packet(b"") is False


def test_is_av1_keyframe_packet_ignores_low_bits() -> None:
    # Low three bits (extension/has-size/reserved) must not affect detection.
    header = _make_obu_header(1) | 0b111
    assert _is_av1_keyframe_packet(bytes([header])) is True


def _avcc_encode(nal_units: list[bytes], nal_length_size: int = 4) -> bytes:
    out = bytearray()
    for unit in nal_units:
        out.extend(len(unit).to_bytes(nal_length_size, "big"))
        out.extend(unit)
    return bytes(out)


def test_avcc_to_annex_b_single_unit() -> None:
    unit = b"\x67\x42\xc0\x1f"
    result = _avcc_to_annex_b(_avcc_encode([unit]))
    assert result == b"\x00\x00\x00\x01" + unit


def test_avcc_to_annex_b_multiple_units() -> None:
    units = [b"\x67\x42\xc0\x1f", b"\x68\xce\x38\x80", b"\x65\x88\x84"]
    result = _avcc_to_annex_b(_avcc_encode(units))
    expected = b"".join(b"\x00\x00\x00\x01" + u for u in units)
    assert result == expected


def test_avcc_to_annex_b_length_size_2() -> None:
    units = [b"\xaa\xbb", b"\xcc\xdd\xee"]
    result = _avcc_to_annex_b(_avcc_encode(units, nal_length_size=2), nal_length_size=2)
    expected = b"".join(b"\x00\x00\x00\x01" + u for u in units)
    assert result == expected


def test_avcc_to_annex_b_truncated_stops_early() -> None:
    # Well-formed first unit, then a length that claims more data than is left.
    first = b"\x67\x42\xc0"
    buf = len(first).to_bytes(4, "big") + first + (0xFF).to_bytes(4, "big") + b"\x00\x01"
    result = _avcc_to_annex_b(buf)
    assert result == b"\x00\x00\x00\x01" + first


def test_avcc_to_annex_b_empty() -> None:
    assert _avcc_to_annex_b(b"") == b""


def test_unwrap_plain_numeric() -> None:
    arr = pa.array([1.0, 2.0, 3.0], type=pa.float64())
    np.testing.assert_array_equal(_unwrap_to_numpy(arr), np.array([1.0, 2.0, 3.0]))


def test_unwrap_list_float() -> None:
    arr = pa.array([[1.0, 2.0], [3.0, 4.0, 5.0]], type=pa.list_(pa.float64()))
    # Non-ragged requirement isn't enforced — the function returns the flattened values.
    np.testing.assert_array_equal(_unwrap_to_numpy(arr), np.array([1.0, 2.0, 3.0, 4.0, 5.0]))


def test_unwrap_fixed_size_list() -> None:
    arr = pa.array([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], type=pa.list_(pa.float32(), 3))
    result = _unwrap_to_numpy(arr)
    np.testing.assert_array_equal(result, np.array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0], dtype=np.float32))


def test_unwrap_nested_list() -> None:
    arr = pa.array([[[1.0, 2.0], [3.0]], [[4.0]]], type=pa.list_(pa.list_(pa.float64())))
    np.testing.assert_array_equal(_unwrap_to_numpy(arr), np.array([1.0, 2.0, 3.0, 4.0]))


def test_unwrap_result_is_writeable() -> None:
    # Torch requires writeable arrays downstream.
    arr = pa.array([1, 2, 3], type=pa.int32())
    result = _unwrap_to_numpy(arr)
    assert result.flags.writeable


def test_flatten_blob_list_of_list_uint8_single_row() -> None:
    arr = pa.array([[[1, 2, 3, 4]]], type=pa.list_(pa.list_(pa.uint8())))
    result = _flatten_blob(arr, 0)
    np.testing.assert_array_equal(result, np.array([1, 2, 3, 4], dtype=np.uint8))


def test_flatten_blob_list_of_list_uint8_concatenates_inner_rows() -> None:
    # Row 0 has two inner lists, which should be concatenated.
    arr = pa.array(
        [[[1, 2], [3]], [[10, 20, 30]]],
        type=pa.list_(pa.list_(pa.uint8())),
    )
    np.testing.assert_array_equal(_flatten_blob(arr, 0), np.array([1, 2, 3], dtype=np.uint8))
    np.testing.assert_array_equal(_flatten_blob(arr, 1), np.array([10, 20, 30], dtype=np.uint8))


def test_flatten_blob_list_of_binary() -> None:
    arr = pa.array([[b"hello"], [b"world!"]], type=pa.list_(pa.binary()))
    np.testing.assert_array_equal(_flatten_blob(arr, 0), np.frombuffer(b"hello", dtype=np.uint8))
    np.testing.assert_array_equal(_flatten_blob(arr, 1), np.frombuffer(b"world!", dtype=np.uint8))


def test_flatten_blob_list_of_large_binary() -> None:
    arr = pa.array([[b"abc"], [b"defghi"]], type=pa.list_(pa.large_binary()))
    np.testing.assert_array_equal(_flatten_blob(arr, 0), np.frombuffer(b"abc", dtype=np.uint8))
    np.testing.assert_array_equal(_flatten_blob(arr, 1), np.frombuffer(b"defghi", dtype=np.uint8))


def test_flatten_blob_binary_respects_offsets() -> None:
    # The binary-path reads raw offsets, make sure subsequent rows don't leak into row 0.
    arr = pa.array(
        [[b"AAAA"], [b"BB"], [b"CCCCCC"]],
        type=pa.list_(pa.binary()),
    )
    for row, expected in enumerate([b"AAAA", b"BB", b"CCCCCC"]):
        np.testing.assert_array_equal(_flatten_blob(arr, row), np.frombuffer(expected, dtype=np.uint8))
