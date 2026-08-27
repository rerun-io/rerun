"""Tests for the low-level helpers in `rerun.experimental.dataloader.decoders`."""

from __future__ import annotations

import io
import pickle
from fractions import Fraction
from typing import cast

import av
import numpy as np
import pyarrow as pa
import pytest
import torch
from PIL import Image
from rerun.experimental import Selector
from rerun.experimental.dataloader import Field, Yuv420Frame
from rerun.experimental.dataloader._sample_index import SegmentMetadata
from rerun.experimental.dataloader._utils import (
    FieldFetchRequest,
    IndexedBlock,
    IndexedGroup,
    Target,
    _decode_field_batch,
    _decode_order,
    _find_segment_boundaries,
    _prior_keyframe,
    _resolve_decode_index_range,
    _resolve_decode_requests,
    _resolve_decode_requests_in_block,
)
from rerun.experimental.dataloader.decoders import (
    DecodeRequest,
    FieldBatch,
    ImageDecoder,
    NumericDecoder,
    VideoFrameDecoder,
)
from rerun.experimental.dataloader.decoders._arrow import _flatten_blob, _unwrap_to_numpy
from rerun.experimental.dataloader.decoders._video import _decoder_name, _extract_video_samples, _starts_with


def _encoder_available(name: str) -> bool:
    """True if this PyAV build can encode with *name*."""
    try:
        av.codec.Codec(name, "w")
    except Exception:
        return False
    return True


def _h264_annex_b(nal_units: list[tuple[int, bytes]], use_4byte: bool = True) -> bytes:
    """Build an Annex B H.264 stream from `(nal_unit_type, payload)` pairs."""
    start = b"\x00\x00\x00\x01" if use_4byte else b"\x00\x00\x01"
    out = bytearray()
    for nal_type, payload in nal_units:
        out.extend(start)
        # nal_ref_idc=3, forbidden_zero_bit=0
        out.append((3 << 5) | (nal_type & 0x1F))
        out.extend(payload)
    return bytes(out)


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


def test_field_batch_requires_an_outer_component_list() -> None:
    with pytest.raises(TypeError, match="outer Arrow List"):
        FieldBatch(column=pa.array([1.0, 2.0], type=pa.float64()))


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


def test_numeric_decoder_returns_none_only_for_ragged_window() -> None:
    column = pa.array([[0.0], [1.0, 2.0], [3.0], [4.0]], type=pa.list_(pa.float64()))
    requests = [
        DecodeRequest(
            sample_position=0,
            segment_id="seg",
            index_value=1,
            decode_row_indices=(0, 1),
            output_row_indices=(0, 1),
            starts_at_keyframe=False,
        ),
        DecodeRequest(
            sample_position=1,
            segment_id="seg",
            index_value=3,
            decode_row_indices=(2, 3),
            output_row_indices=(2, 3),
            starts_at_keyframe=False,
        ),
    ]

    ragged, rectangular = NumericDecoder().decode(FieldBatch(column=column, is_windowed=True), requests)

    assert ragged is None
    assert rectangular is not None
    torch.testing.assert_close(rectangular, torch.tensor([[3.0], [4.0]], dtype=torch.float64))


def test_numeric_decoder_returns_none_for_window_with_null_row() -> None:
    column = pa.array([[0.0], None], type=pa.list_(pa.float64()))
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=1,
        decode_row_indices=(0, 1),
        output_row_indices=(0, 1),
        starts_at_keyframe=False,
    )

    (decoded,) = NumericDecoder().decode(FieldBatch(column=column, is_windowed=True), [request])

    assert decoded is None


def test_numeric_decoder_allows_different_widths_across_rectangular_window_requests() -> None:
    column = pa.array([[0.0], [1.0], [2.0, 3.0], [4.0, 5.0]], type=pa.list_(pa.float64()))
    requests = [
        DecodeRequest(
            sample_position=0,
            segment_id="seg-a",
            index_value=1,
            decode_row_indices=(0, 1),
            output_row_indices=(0, 1),
            starts_at_keyframe=False,
        ),
        DecodeRequest(
            sample_position=0,
            segment_id="seg-b",
            index_value=1,
            decode_row_indices=(2, 3),
            output_row_indices=(2, 3),
            starts_at_keyframe=False,
        ),
    ]

    narrow, wide = NumericDecoder().decode(FieldBatch(column=column, is_windowed=True), requests)

    assert narrow is not None and wide is not None
    torch.testing.assert_close(narrow, torch.tensor([[0.0], [1.0]], dtype=torch.float64))
    torch.testing.assert_close(wide, torch.tensor([[2.0, 3.0], [4.0, 5.0]], dtype=torch.float64))


@pytest.mark.parametrize("dims", [1, 7])
def test_numeric_decoder_window_preserves_time_and_value_axes(dims: int) -> None:
    rows = [[float(observation * 10 + dimension) for dimension in range(dims)] for observation in range(4)]
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=3,
        decode_row_indices=(0, 1, 2, 3),
        output_row_indices=(0, 1, 2, 3),
        starts_at_keyframe=False,
    )

    (decoded,) = NumericDecoder().decode(
        FieldBatch(column=pa.array(rows, type=pa.list_(pa.float64())), is_windowed=True),
        [request],
    )

    assert decoded is not None
    assert tuple(decoded.shape) == (4, dims)
    torch.testing.assert_close(decoded, torch.tensor(rows, dtype=torch.float64))


def test_numeric_decoder_window_preserves_value_axis_after_scalar_selector() -> None:
    column = pa.array(
        [[{"value": 1.0}], [{"value": 2.0}]],
        type=pa.list_(pa.struct({"value": pa.float64()})),
    )
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=1,
        decode_row_indices=(0, 1),
        output_row_indices=(0, 1),
        starts_at_keyframe=False,
    )

    (decoded,) = NumericDecoder().decode(
        FieldBatch(column=column, select=Selector(".[0].value"), is_windowed=True),
        [request],
    )

    assert decoded is not None
    assert tuple(decoded.shape) == (2, 1)
    torch.testing.assert_close(decoded, torch.tensor([[1.0], [2.0]], dtype=torch.float64))


def test_numeric_decoder_window_repeats_latest_row() -> None:
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=10,
        decode_row_indices=(0, 1),
        output_row_indices=(0, 0, 1),
        starts_at_keyframe=False,
    )

    (decoded,) = NumericDecoder().decode(
        FieldBatch(column=pa.array([[1.0], [2.0]], type=pa.list_(pa.float64())), is_windowed=True),
        [request],
    )

    assert decoded is not None
    torch.testing.assert_close(decoded, torch.tensor([[1.0], [1.0], [2.0]], dtype=torch.float64))


def test_numeric_decoder_uniform_rows_gather_requests_together() -> None:
    requests = [
        DecodeRequest(
            sample_position=0,
            segment_id="seg",
            index_value=1,
            decode_row_indices=(0, 1),
            output_row_indices=(0, 1),
            starts_at_keyframe=False,
        ),
        DecodeRequest(
            sample_position=0,
            segment_id="seg",
            index_value=3,
            decode_row_indices=(2, 3),
            output_row_indices=(2, 3),
            starts_at_keyframe=False,
        ),
    ]
    rows = [[0.0, 1.0], [2.0, 3.0], [4.0, 5.0], [6.0, 7.0]]

    decoded = NumericDecoder().decode(
        FieldBatch(column=pa.array(rows, type=pa.list_(pa.float64())), is_windowed=True), requests
    )

    assert decoded[0] is not None and decoded[1] is not None
    torch.testing.assert_close(decoded[0], torch.tensor(rows[:2], dtype=torch.float64))
    torch.testing.assert_close(decoded[1], torch.tensor(rows[2:], dtype=torch.float64))


def test_numeric_decoder_variable_width_rows_use_ragged_fallback() -> None:
    requests = [
        DecodeRequest(
            sample_position=0,
            segment_id="seg",
            index_value=row,
            decode_row_indices=(row,),
            output_row_indices=(row,),
            starts_at_keyframe=False,
        )
        for row in range(3)
    ]
    rows = [[0.0], [1.0, 2.0], [3.0, 4.0, 5.0]]

    decoded = NumericDecoder().decode(FieldBatch(column=pa.array(rows, type=pa.list_(pa.float64()))), requests)

    assert all(tensor is not None for tensor in decoded)
    for tensor, expected in zip(decoded, rows, strict=True):
        torch.testing.assert_close(tensor, torch.tensor(expected, dtype=torch.float64))


def test_numeric_decoder_preserves_empty_unwindowed_value() -> None:
    column = pa.array([[], [1.0, 2.0]], type=pa.list_(pa.float64()))
    requests = [
        DecodeRequest(
            sample_position=0,
            segment_id="seg",
            index_value=row,
            decode_row_indices=(row,),
            output_row_indices=(row,),
            starts_at_keyframe=False,
        )
        for row in range(2)
    ]

    decoded = NumericDecoder().decode(FieldBatch(column=column), requests)

    assert decoded[0] is not None and decoded[0].numel() == 0
    assert decoded[1] is not None
    torch.testing.assert_close(decoded[1], torch.tensor([1.0, 2.0], dtype=torch.float64))


def test_image_decoder_window_returns_every_frame() -> None:
    encoded: list[list[bytes]] = []
    for shade in (10, 20, 30):
        buffer = io.BytesIO()
        Image.new("RGB", (4, 3), (shade, shade, shade)).save(buffer, format="PNG")
        encoded.append([buffer.getvalue()])
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=2,
        decode_row_indices=(0, 1, 2),
        output_row_indices=(0, 1, 2),
        starts_at_keyframe=False,
    )

    (decoded,) = ImageDecoder().decode(
        FieldBatch(column=pa.array(encoded, type=pa.list_(pa.binary())), is_windowed=True),
        [request],
    )

    assert decoded is not None
    assert tuple(decoded.shape) == (3, 3, 3, 4)
    assert [int(frame[0, 0, 0]) for frame in decoded] == [10, 20, 30]


def test_image_decoder_returns_none_for_corrupt_data_without_losing_other_requests() -> None:
    buffer = io.BytesIO()
    Image.new("RGB", (4, 3), (10, 10, 10)).save(buffer, format="PNG")
    column = pa.array([[b"not an image"], [buffer.getvalue()]], type=pa.list_(pa.binary()))
    requests = [
        DecodeRequest(
            sample_position=row,
            segment_id="seg",
            index_value=row,
            decode_row_indices=(row,),
            output_row_indices=(row,),
            starts_at_keyframe=False,
        )
        for row in range(2)
    ]

    corrupt, valid = ImageDecoder().decode(FieldBatch(column=column), requests)

    assert corrupt is None
    assert valid is not None
    assert tuple(valid.shape) == (3, 3, 4)


def test_video_frame_decoder_returns_none_without_keyframe() -> None:
    """`decode` returns `None` when the prefetched window contains no keyframe."""
    p_slice_only = _h264_annex_b([(1, b"\xab\xcd\xef\x01\x02\x03")])

    decoder = VideoFrameDecoder(codec="h264")
    assert _decode_window(decoder, [p_slice_only], 0) is None


def test_extract_video_samples_preserves_identical_packets() -> None:
    packet = b"same encoded packet"
    column = pa.array([[packet], [packet]], type=pa.list_(pa.binary()))

    samples, rows = _extract_video_samples(column, range(2), video_codec=None)

    assert samples == [packet, packet]
    assert rows == [0, 1]


def test_video_frame_decoder_skips_request_without_resolved_keyframe() -> None:
    samples = _encode_h264(num_frames=4, gop=4)
    unresolved = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=1,
        decode_row_indices=(0, 1),
        output_row_indices=(1,),
        starts_at_keyframe=False,
    )
    resolved = DecodeRequest(
        sample_position=1,
        segment_id="seg",
        index_value=3,
        decode_row_indices=(0, 1, 2, 3),
        output_row_indices=(3,),
        starts_at_keyframe=True,
    )

    decoded = VideoFrameDecoder(codec="h264").decode(_sample_batch(samples), [unresolved, resolved])

    assert decoded[0] is None
    assert decoded[1] is not None


def test_video_frame_decoder_returns_none_for_one_corrupt_gop_and_continues(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    packet = _h264_annex_b([(5, b"\x88")])
    decoder = VideoFrameDecoder(codec="h264")
    monkeypatch.setattr(decoder, "_is_keyframe", lambda _sample: True)

    def feed_run(
        segment_id: str,
        _feed: list[bytes],
        wanted: list[int],
        *,
        capture_frame: object | None = None,
    ) -> dict[int, torch.Tensor]:
        del capture_frame
        if segment_id == "corrupt":
            raise av.error.InvalidDataError(1, "corrupt packet")
        return {wanted[-1]: torch.zeros((3, 2, 2), dtype=torch.uint8)}

    monkeypatch.setattr(decoder, "_feed_run", feed_run)
    requests = [
        DecodeRequest(
            sample_position=0,
            segment_id="corrupt",
            index_value=0,
            decode_row_indices=(0,),
            output_row_indices=(0,),
            starts_at_keyframe=True,
        ),
        DecodeRequest(
            sample_position=1,
            segment_id="valid",
            index_value=0,
            decode_row_indices=(1,),
            output_row_indices=(1,),
            starts_at_keyframe=True,
        ),
    ]

    corrupt, valid = decoder.decode(_sample_batch([packet, packet]), requests)

    assert corrupt is None
    assert valid is not None


def test_video_frame_decoder_is_keyframe_h264() -> None:
    gop = 4
    samples = _encode_h264(num_frames=8, gop=gop)
    decoder = VideoFrameDecoder(codec="h264")
    assert decoder._is_keyframe(samples[0]) is True
    assert decoder._is_keyframe(samples[1]) is False
    assert decoder._is_keyframe(samples[gop]) is True


def test_video_frame_decoder_is_keyframe_h264_idr_without_sps() -> None:
    # An IDR NAL alone can't bootstrap a decoder (no SPS): not a keyframe.
    idr_only = _h264_annex_b([(5, b"\x88")])
    assert VideoFrameDecoder(codec="h264")._is_keyframe(idr_only) is False


@pytest.mark.skipif(not _encoder_available("libx265"), reason="PyAV build lacks the libx265 encoder")
def test_video_frame_decoder_is_keyframe_hevc() -> None:
    samples = _encode_hevc(num_frames=4, gop=4)
    decoder = VideoFrameDecoder(codec="hevc")
    assert decoder._is_keyframe(samples[0]) is True
    assert decoder._is_keyframe(samples[1]) is False


def test_video_frame_decoder_is_keyframe_undetectable_codec_returns_none() -> None:
    assert VideoFrameDecoder(codec="mjpeg")._is_keyframe(b"\x00") is None


def test_video_frame_decoder_is_keyframe_vp9_classifies_garbage() -> None:
    # vp9 has a detector, so garbage is classified rather than passed through as None.
    assert VideoFrameDecoder(codec="vp9")._is_keyframe(b"\x00") is False


def test_video_frame_decoder_derives_keyframe_path() -> None:
    decoder = VideoFrameDecoder(codec="h264")
    assert decoder.prior_keyframe_path("/camera:VideoStream:sample") == "/camera:VideoStream:is_keyframe"
    assert (
        decoder.prior_keyframe_path("/robot/cam_left:VideoStream:sample") == "/robot/cam_left:VideoStream:is_keyframe"
    )


def test_video_frame_decoder_keyframe_path_no_separator() -> None:
    # Defensive: a path with no `:` is non-canonical; return None rather than guessing.
    assert VideoFrameDecoder(codec="h264").prior_keyframe_path("/just_an_entity") is None


def test_resolve_decode_index_range_combines_window_outputs_with_anchor() -> None:
    field = Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder(codec="h264"), window=(-3, 5))
    assert _resolve_decode_index_range(100, field, output_index_values=(97, 105), prior_keyframe=42) == (42, 105)


def test_resolve_decode_index_range_uses_prior_keyframe_integer() -> None:
    field = Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder(codec="h264"))
    assert _resolve_decode_index_range(100, field, output_index_values=(100,), prior_keyframe=87) == (87, 100)


def test_resolve_decode_index_range_uses_prior_keyframe_timestamp() -> None:
    field = Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder(codec="h264"))
    target = np.datetime64(1_000_000_000, "ns")
    result = _resolve_decode_index_range(
        target,
        field,
        output_index_values=(target,),
        prior_keyframe=500_000_000,
    )
    assert result is not None
    lo, hi = result
    assert lo == np.datetime64(500_000_000, "ns")
    assert hi == target


def test_prior_keyframe_none_or_empty_returns_none() -> None:
    assert _prior_keyframe(None, 100) is None
    assert _prior_keyframe(np.array([], dtype=np.int64), 100) is None


def test_prior_keyframe_target_before_first_returns_none() -> None:
    assert _prior_keyframe(np.array([50, 100, 150], dtype=np.int64), 49) is None


def test_prior_keyframe_target_equals_keyframe_returns_keyframe() -> None:
    assert _prior_keyframe(np.array([50, 100, 150], dtype=np.int64), 100) == 100


def test_prior_keyframe_target_between_returns_largest_leq() -> None:
    kfs = np.array([50, 100, 150], dtype=np.int64)
    assert _prior_keyframe(kfs, 99) == 50
    assert _prior_keyframe(kfs, 149) == 100


def test_prior_keyframe_target_after_last_returns_last() -> None:
    assert _prior_keyframe(np.array([50, 100, 150], dtype=np.int64), 9999) == 150


def test_starts_with() -> None:
    assert _starts_with([b"a", b"b", b"c"], [])
    assert _starts_with([b"a", b"b", b"c"], [b"a", b"b"])
    assert _starts_with([b"a", b"b"], [b"a", b"b"])
    assert not _starts_with([b"a"], [b"a", b"b"])
    assert not _starts_with([b"a", b"x"], [b"a", b"b"])


@pytest.mark.parametrize(("codec", "expected"), [("AVC", "h264"), ("H265", "hevc"), ("HEVC", "hevc")])
def test_video_decoder_name_normalizes_aliases(codec: str, expected: str) -> None:
    assert _decoder_name(codec) == expected


def _encode_h264(num_frames: int, gop: int, b_frames: int = 0) -> list[bytes]:
    """One Annex B sample per frame, keyframes every *gop* frames."""
    encoder = av.CodecContext.create("libx264", "w")
    encoder.width, encoder.height = 64, 64
    encoder.pix_fmt = "yuv420p"
    encoder.time_base = Fraction(1, 30)
    encoder.framerate = Fraction(30, 1)
    encoder.options = {"g": str(gop), "bf": str(b_frames), "tune": "zerolatency" if b_frames == 0 else "psnr"}
    samples: list[bytes] = []
    for i in range(num_frames):
        pixels = np.empty((64, 64, 3), dtype=np.uint8)
        pixels[:, :, 0] = ((np.arange(64) + i) % 256)[np.newaxis, :]
        pixels[:, :, 1] = ((np.arange(64) + i * 3) % 256)[:, np.newaxis]
        pixels[:, :, 2] = (i * 7) % 256
        frame = av.VideoFrame.from_ndarray(pixels, format="rgb24").reformat(format="yuv420p")
        frame.pts = i
        samples.extend(bytes(p) for p in encoder.encode(frame))
    samples.extend(bytes(p) for p in encoder.encode(None))
    assert len(samples) == num_frames
    return samples


def _encode_hevc(num_frames: int, gop: int) -> list[bytes]:
    """One Annex B HEVC sample per frame, keyframes every *gop* frames, headers repeated on each keyframe."""
    # The PyAV stubs' video-codec-name literal doesn't know libx265, so the overload needs help.
    encoder = cast("av.VideoCodecContext", av.CodecContext.create("libx265", "w"))
    encoder.width, encoder.height = 64, 64
    encoder.pix_fmt = "yuv420p"
    encoder.time_base = Fraction(1, 30)
    encoder.framerate = Fraction(30, 1)
    encoder.options = {
        "x265-params": f"keyint={gop}:min-keyint={gop}:bframes=0:repeat-headers=1:log-level=none",
    }
    samples: list[bytes] = []
    for i in range(num_frames):
        pixels = np.full((64, 64, 3), (i * 31) % 256, dtype=np.uint8)
        frame = av.VideoFrame.from_ndarray(pixels, format="rgb24").reformat(format="yuv420p")
        frame.pts = i
        samples.extend(bytes(p) for p in encoder.encode(frame))
    samples.extend(bytes(p) for p in encoder.encode(None))
    assert len(samples) == num_frames
    return samples


def _sample_batch(samples: list[bytes], *, is_windowed: bool = False) -> FieldBatch:
    """A `FieldBatch` over encoded samples, one row per sample."""
    column = pa.array([[s] for s in samples], type=pa.list_(pa.binary()))
    return FieldBatch(column=column, is_windowed=is_windowed)


def _decode_one(
    decoder: VideoFrameDecoder,
    samples: list[bytes],
    target: int,
    segment_id: str = "seg",
    *,
    window_start: int = 0,
) -> torch.Tensor | None:
    """Decode the frame at row *target* from *samples*, with the decode window anchored at row *window_start*."""
    request = DecodeRequest(
        sample_position=0,
        segment_id=segment_id,
        index_value=target,
        decode_row_indices=tuple(range(window_start, target + 1)),
        output_row_indices=(target,),
        starts_at_keyframe=True,
    )
    return decoder.decode(_sample_batch(samples), [request])[0]


def _decode_window(decoder: VideoFrameDecoder, samples: list[bytes], target: int) -> torch.Tensor | None:
    """Decode *samples* as one pre-sliced window whose last row holds the target frame."""
    del target
    return _decode_one(decoder, samples, len(samples) - 1)


def _session_contexts(decoder: VideoFrameDecoder) -> list[av.VideoCodecContext]:
    return [session.context for session in decoder._sessions.values()]


def test_video_frame_decoder_sequential_reads_reuse_session() -> None:
    gop = 6
    samples = _encode_h264(num_frames=12, gop=gop)
    decoder = VideoFrameDecoder(codec="h264")

    contexts = []
    for target in range(12):
        keyframe = (target // gop) * gop
        got = _decode_one(decoder, samples, target, window_start=keyframe)
        expected = _decode_one(VideoFrameDecoder(codec="h264"), samples, target, window_start=keyframe)
        assert got is not None and expected is not None
        assert torch.equal(got, expected)
        contexts.extend(_session_contexts(decoder))

    # One context per GOP; without sessions this would be one per target.
    assert len(set(map(id, contexts))) == 2


def test_video_frame_decoder_repeated_target_hits_session() -> None:
    samples = _encode_h264(num_frames=4, gop=4)
    decoder = VideoFrameDecoder(codec="h264")

    first = _decode_one(decoder, samples, 2)
    context = _session_contexts(decoder)[0]
    second = _decode_one(decoder, samples, 2)
    assert first is not None and second is not None
    assert torch.equal(first, second)
    assert _session_contexts(decoder) == [context]


def test_video_frame_decoder_backward_step_restarts_session() -> None:
    gop = 6
    samples = _encode_h264(num_frames=6, gop=gop)
    decoder = VideoFrameDecoder(codec="h264")

    _decode_one(decoder, samples, 4)
    context = _session_contexts(decoder)[0]
    # A target before the session's frontier was already discarded: a fresh context must replay it.
    got = _decode_one(decoder, samples, 2)
    expected = _decode_one(VideoFrameDecoder(codec="h264"), samples, 2)
    assert got is not None and expected is not None
    assert torch.equal(got, expected)
    assert _session_contexts(decoder) != [context]


def test_video_frame_decoder_segments_get_separate_sessions() -> None:
    samples = _encode_h264(num_frames=4, gop=4)
    decoder = VideoFrameDecoder(codec="h264")

    a = _decode_one(decoder, samples, 1, segment_id="seg_a")
    b = _decode_one(decoder, samples, 1, segment_id="seg_b")
    assert a is not None and b is not None
    assert torch.equal(a, b)
    assert len(decoder._sessions) == 2


def test_video_frame_decoder_delayed_stream_falls_back_to_flush() -> None:
    # B-frames make the decoder hold frames back, so no session can be kept.
    samples = _encode_h264(num_frames=8, gop=8, b_frames=2)
    decoder = VideoFrameDecoder(codec="h264")

    assert _decode_one(decoder, samples, 7) is not None
    assert len(decoder._sessions) == 0


@pytest.mark.parametrize("thread_count", [2, 4])
def test_video_frame_decoder_frame_threading_drains_contiguous_run_once(
    monkeypatch: pytest.MonkeyPatch,
    thread_count: int,
) -> None:
    samples = _encode_h264(num_frames=16, gop=16)
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=13,
        decode_row_indices=tuple(range(14)),
        output_row_indices=tuple(range(8, 14)),
        starts_at_keyframe=True,
    )
    batch = _sample_batch(samples, is_windowed=True)
    expected = VideoFrameDecoder(
        codec="h264",
        window_storage="view",
    ).decode(batch, [request])[0]
    decoder = VideoFrameDecoder(
        codec="h264",
        thread_count=thread_count,
        window_storage="view",
    )

    def unexpected_replay(*_args: object, **_kwargs: object) -> torch.Tensor:
        raise AssertionError("frame-threaded contiguous decode must not replay each requested frame")

    monkeypatch.setattr(decoder, "_feed_last", unexpected_replay)
    actual = decoder.decode(batch, [request])[0]

    assert actual is not None and expected is not None
    torch.testing.assert_close(actual, expected)
    assert actual.is_contiguous()
    assert len(decoder._sessions) == 0


def test_video_frame_decoder_yuv420_view_handles_delayed_stream() -> None:
    samples = _encode_h264(num_frames=8, gop=8, b_frames=2)
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=7,
        decode_row_indices=tuple(range(8)),
        output_row_indices=(5, 6, 7),
        starts_at_keyframe=True,
    )
    batch = _sample_batch(samples, is_windowed=True)

    (expected,) = VideoFrameDecoder(codec="h264", window_storage="view").decode(batch, [request])
    (decoded,) = VideoFrameDecoder(
        codec="h264",
        window_storage="view",
        output_format="yuv420p",
    ).decode(batch, [request])

    assert isinstance(expected, torch.Tensor)
    assert isinstance(decoded, Yuv420Frame)
    converted = decoded.to_rgb(normalize=False, color_space="bt601", color_range="limited")
    torch.testing.assert_close(converted, expected.float(), rtol=0, atol=5)


def test_video_frame_decoder_pickle_drops_sessions() -> None:
    samples = _encode_h264(num_frames=4, gop=4)
    decoder = VideoFrameDecoder(codec="h264")
    assert _decode_one(decoder, samples, 1) is not None
    assert len(decoder._sessions) == 1

    restored = pickle.loads(pickle.dumps(decoder))
    assert len(restored._sessions) == 0
    assert _decode_one(restored, samples, 1) is not None


def test_video_frame_decoder_duplicate_slots_do_not_share_memory() -> None:
    # Two requests snapping to the same kept sample (a fixed-rate grid denser than
    # the video fps): equal content, but mutating one must not corrupt the other.
    samples = _encode_h264(num_frames=4, gop=4)
    decoder = VideoFrameDecoder(codec="h264")
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=3,
        decode_row_indices=tuple(range(4)),
        output_row_indices=(3,),
        starts_at_keyframe=True,
    )

    out = decoder.decode(_sample_batch(samples), [request, request])

    assert out[0] is not None and out[1] is not None
    assert torch.equal(out[0], out[1])
    assert out[0].data_ptr() != out[1].data_ptr()


def test_video_frame_decoder_returns_window_stack() -> None:
    samples = _encode_h264(num_frames=8, gop=8)
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=5,
        decode_row_indices=(0, 1, 2, 3, 4, 5),
        output_row_indices=(2, 3, 4, 5),
        starts_at_keyframe=True,
    )

    (decoded,) = VideoFrameDecoder(codec="h264").decode(_sample_batch(samples, is_windowed=True), [request])

    assert decoded is not None
    assert tuple(decoded.shape[:2]) == (4, 3)
    for slot, target in enumerate(request.output_row_indices):
        expected = _decode_one(VideoFrameDecoder(codec="h264"), samples, target)
        assert expected is not None
        assert torch.equal(decoded[slot], expected)


def test_video_frame_decoder_window_stacks_do_not_share_memory() -> None:
    samples = _encode_h264(num_frames=4, gop=4)
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=3,
        decode_row_indices=(0, 1, 2, 3),
        output_row_indices=(2, 3),
        starts_at_keyframe=True,
    )

    first, second = VideoFrameDecoder(codec="h264").decode(
        _sample_batch(samples, is_windowed=True),
        [request, request],
    )

    assert first is not None and second is not None
    assert torch.equal(first, second)
    assert first.data_ptr() != second.data_ptr()
    first.zero_()
    assert torch.count_nonzero(second) > 0


def test_video_frame_decoder_contiguous_windows_can_share_frame_bank() -> None:
    samples = _encode_h264(num_frames=5, gop=5)
    requests = [
        DecodeRequest(
            sample_position=position,
            segment_id="seg",
            index_value=position + 2,
            decode_row_indices=tuple(range(position + 3)),
            output_row_indices=(position + 1, position + 2),
            starts_at_keyframe=True,
        )
        for position in range(2)
    ]

    batch = _sample_batch(samples, is_windowed=True)
    expected = VideoFrameDecoder(codec="h264").decode(batch, requests)
    first, second = VideoFrameDecoder(codec="h264", window_storage="view").decode(
        batch,
        requests,
    )

    assert first is not None and second is not None
    assert expected[0] is not None and expected[1] is not None
    torch.testing.assert_close(first, expected[0])
    torch.testing.assert_close(second, expected[1])
    assert first.untyped_storage().data_ptr() == second.untyped_storage().data_ptr()
    first[1].zero_()
    assert torch.count_nonzero(second[0]) == 0


def test_video_frame_decoder_yuv420_writes_shared_bank_directly(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    samples = _encode_h264(num_frames=5, gop=5)
    requests = [
        DecodeRequest(
            sample_position=position,
            segment_id="seg",
            index_value=position + 2,
            decode_row_indices=tuple(range(position + 3)),
            output_row_indices=(position + 1, position + 2),
            starts_at_keyframe=True,
        )
        for position in range(2)
    ]
    batch = _sample_batch(samples, is_windowed=True)
    stack_calls = 0
    original_stack = torch.stack

    def count_stack(*args: object, **kwargs: object) -> torch.Tensor:
        nonlocal stack_calls
        stack_calls += 1
        return original_stack(*args, **kwargs)  # type: ignore[arg-type]

    monkeypatch.setattr(torch, "stack", count_stack)
    first, second = VideoFrameDecoder(
        codec="h264",
        window_storage="view",
        output_format="yuv420p",
    ).decode(batch, requests)

    assert isinstance(first, Yuv420Frame) and isinstance(second, Yuv420Frame)
    assert first.y.is_contiguous() and first.uv.is_contiguous()
    assert second.y.is_contiguous() and second.uv.is_contiguous()
    assert first.y.untyped_storage().data_ptr() == second.y.untyped_storage().data_ptr()
    assert first.uv.untyped_storage().data_ptr() == second.uv.untyped_storage().data_ptr()
    assert stack_calls == 0


def test_video_frame_decoder_yuv420_matches_rgb_conversion() -> None:
    samples = _encode_h264(num_frames=8, gop=8)
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=7,
        decode_row_indices=tuple(range(8)),
        output_row_indices=tuple(range(2, 8)),
        starts_at_keyframe=True,
    )
    batch = _sample_batch(samples, is_windowed=True)
    (expected,) = VideoFrameDecoder(codec="h264", window_storage="view").decode(batch, [request])
    (decoded,) = VideoFrameDecoder(
        codec="h264",
        window_storage="view",
        output_format="yuv420p",
    ).decode(batch, [request])

    assert isinstance(expected, torch.Tensor)
    assert isinstance(decoded, Yuv420Frame)
    assert decoded.y.shape == (6, 1, 64, 64)
    assert decoded.uv.shape == (6, 2, 32, 32)
    assert decoded.y.untyped_storage().nbytes() + decoded.uv.untyped_storage().nbytes() == expected.numel() // 2
    converted = decoded.to_rgb(
        dtype=torch.float32,
        normalize=False,
        color_space="bt601",
        color_range="limited",
    )
    torch.testing.assert_close(converted, expected.float(), rtol=0, atol=3)


@pytest.mark.parametrize(
    ("av_color_space", "expected"),
    [(1, "bt709"), (5, "bt601"), (6, "bt601"), (9, "bt2020"), (2, "unspecified")],
)
def test_video_frame_decoder_maps_yuv_color_space_metadata(av_color_space: int, expected: str) -> None:
    frame = av.VideoFrame(4, 4, "yuv420p")
    frame.colorspace = av_color_space

    assert VideoFrameDecoder._frame_color_space(frame) == expected


def test_video_frame_decoder_preserves_full_range_yuvj420p_samples() -> None:
    frame = av.VideoFrame(4, 4, "yuvj420p")
    for plane, value in zip(frame.planes, (32, 96, 160), strict=True):
        plane.update(bytes([value]) * plane.buffer_size)

    decoded = VideoFrameDecoder(output_format="yuv420p")._frame_to_yuv420(frame)

    assert decoded.color_range == "full"
    assert torch.all(decoded.y == 32)
    assert torch.all(decoded.uv[0] == 96)
    assert torch.all(decoded.uv[1] == 160)


def test_video_frame_decoder_yuv420_contiguous_windows_share_plane_banks() -> None:
    samples = _encode_h264(num_frames=5, gop=5)
    requests = [
        DecodeRequest(
            sample_position=position,
            segment_id="seg",
            index_value=position + 2,
            decode_row_indices=tuple(range(position + 3)),
            output_row_indices=(position + 1, position + 2),
            starts_at_keyframe=True,
        )
        for position in range(2)
    ]
    first, second = VideoFrameDecoder(
        codec="h264",
        window_storage="view",
        output_format="yuv420p",
    ).decode(_sample_batch(samples, is_windowed=True), requests)

    assert isinstance(first, Yuv420Frame) and isinstance(second, Yuv420Frame)
    assert first.y.untyped_storage().data_ptr() == second.y.untyped_storage().data_ptr()
    assert first.uv.untyped_storage().data_ptr() == second.uv.untyped_storage().data_ptr()


def test_video_frame_decoder_collation_materializes_view_windows() -> None:
    samples = _encode_h264(num_frames=5, gop=5)
    requests = [
        DecodeRequest(
            sample_position=position,
            segment_id="seg",
            index_value=position + 2,
            decode_row_indices=tuple(range(position + 3)),
            output_row_indices=(position + 1, position + 2),
            starts_at_keyframe=True,
        )
        for position in range(2)
    ]
    first, second = VideoFrameDecoder(codec="h264", window_storage="view").decode(
        _sample_batch(samples, is_windowed=True),
        requests,
    )

    assert first is not None and second is not None
    collated = torch.stack([first, second])
    first[1].zero_()
    assert torch.count_nonzero(collated[0, 1]) > 0
    assert torch.count_nonzero(collated[1, 0]) > 0


def test_video_frame_decoder_view_mode_copies_irregular_windows() -> None:
    samples = _encode_h264(num_frames=5, gop=5)
    irregular = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=3,
        decode_row_indices=(0, 1, 2, 3),
        output_row_indices=(1, 3),
        starts_at_keyframe=True,
    )
    middle = DecodeRequest(
        sample_position=1,
        segment_id="seg",
        index_value=2,
        decode_row_indices=(0, 1, 2),
        output_row_indices=(2,),
        starts_at_keyframe=True,
    )
    last = DecodeRequest(
        sample_position=2,
        segment_id="seg",
        index_value=3,
        decode_row_indices=(0, 1, 2, 3),
        output_row_indices=(3,),
        starts_at_keyframe=True,
    )

    batch = _sample_batch(samples, is_windowed=True)
    requests = [irregular, middle, last]
    expected = VideoFrameDecoder(codec="h264").decode(batch, requests)
    irregular_out, middle_out, last_out = VideoFrameDecoder(codec="h264", window_storage="view").decode(
        batch,
        requests,
    )

    assert irregular_out is not None and middle_out is not None and last_out is not None
    for actual, copied in zip((irregular_out, middle_out, last_out), expected, strict=True):
        assert copied is not None
        torch.testing.assert_close(actual, copied)
    assert middle_out.untyped_storage().data_ptr() == last_out.untyped_storage().data_ptr()
    assert irregular_out.untyped_storage().data_ptr() != last_out.untyped_storage().data_ptr()
    irregular_out[1].zero_()
    assert torch.count_nonzero(last_out[0]) > 0


def test_video_frame_decoder_view_mode_skips_bank_without_contiguous_windows(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    samples = _encode_h264(num_frames=4, gop=4)
    request = DecodeRequest(
        sample_position=0,
        segment_id="seg",
        index_value=2,
        decode_row_indices=(0, 1, 2),
        output_row_indices=(2, 2),
        starts_at_keyframe=True,
    )
    stack_calls = 0
    original_stack = torch.stack

    def count_stack(*args: object, **kwargs: object) -> torch.Tensor:
        nonlocal stack_calls
        stack_calls += 1
        return original_stack(*args, **kwargs)  # type: ignore[arg-type]

    monkeypatch.setattr(torch, "stack", count_stack)
    (decoded,) = VideoFrameDecoder(codec="h264", window_storage="view").decode(
        _sample_batch(samples, is_windowed=True),
        [request],
    )

    assert decoded is not None
    assert stack_calls == 1


def test_video_frame_decoder_rejects_unknown_window_storage() -> None:
    with pytest.raises(ValueError, match="window_storage"):
        VideoFrameDecoder(window_storage="unknown")  # type: ignore[call-overload]


def test_video_frame_decoder_rejects_unknown_output_format() -> None:
    with pytest.raises(ValueError, match="output_format"):
        VideoFrameDecoder(output_format="unknown")  # type: ignore[call-overload]


def test_video_frame_decoder_accepts_automatic_thread_count() -> None:
    decoder = VideoFrameDecoder(thread_count=0)

    assert decoder.thread_count == 0
    assert decoder._create_context().thread_count == 0


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"thread_count": -1}, "thread_count"),
        ({"max_decoder_sessions": -1}, "max_decoder_sessions"),
    ],
)
def test_video_frame_decoder_rejects_invalid_resource_limits(kwargs: dict[str, int], message: str) -> None:
    with pytest.raises(ValueError, match=message):
        VideoFrameDecoder(**kwargs)  # type: ignore[call-overload]


def _group_table(segment_ids: list[str], index_values: list[int]) -> pa.Table:
    """A read-group table with one `x` value per row, equal to that row's index value."""
    return pa.table({
        "t": pa.array(index_values, pa.int64()),
        "rerun_segment_id": pa.array(segment_ids, pa.string()),
        "x": pa.array([float(v) for v in index_values], pa.float64()),
    })


def _targets(pairs: list[tuple[str, int]]) -> list[Target]:
    return [
        Target(
            segment=SegmentMetadata(segment_id=segment_id, index_start=0, index_end=0, num_samples=0),
            index_value=index_value,
            fetch_requests={
                "x": FieldFetchRequest(
                    sample_position=position,
                    segment_id=segment_id,
                    index_value=index_value,
                    decode_index_range=(index_value, index_value),
                    output_index_values=(index_value,),
                    fill_latest_at=False,
                    requires_contiguous_fetch=False,
                    starts_at_keyframe=False,
                )
            },
        )
        for position, (segment_id, index_value) in enumerate(pairs)
    ]


def test_find_segment_boundaries_leaves_ordered_rows_alone() -> None:
    indexed_table = _find_segment_boundaries(_group_table(["a", "a", "b", "b", "b"], [0, 1, 100, 101, 102]), "t")

    assert indexed_table.segment_spans == {"a": (0, 2), "b": (2, 5)}
    np.testing.assert_array_equal(indexed_table.index_values, [0, 1, 100, 101, 102])
    np.testing.assert_array_equal(indexed_table.table.column("x").to_numpy(), [0.0, 1.0, 100.0, 101.0, 102.0])


def test_find_segment_boundaries_allows_index_values_to_restart_per_segment() -> None:
    # `b`'s values are below `a`'s. Expected — they are different timelines — and not a reason to sort.
    indexed_table = _find_segment_boundaries(_group_table(["a", "a", "b", "b"], [50, 60, 0, 10]), "t")

    assert indexed_table.segment_spans == {"a": (0, 2), "b": (2, 4)}
    np.testing.assert_array_equal(indexed_table.index_values, [50, 60, 0, 10])


def test_find_segment_boundaries_sorts_a_descending_segment() -> None:
    indexed_table = _find_segment_boundaries(_group_table(["a", "a", "a"], [2, 0, 1]), "t")

    assert indexed_table.segment_spans == {"a": (0, 3)}
    np.testing.assert_array_equal(indexed_table.index_values, [0, 1, 2])
    np.testing.assert_array_equal(indexed_table.table.column("x").to_numpy(), [0.0, 1.0, 2.0])


def test_find_segment_boundaries_gathers_a_split_segment() -> None:
    indexed_table = _find_segment_boundaries(_group_table(["a", "b", "a"], [0, 5, 1]), "t")

    assert set(indexed_table.segment_spans) == {"a", "b"}
    for segment_id, (start, stop) in indexed_table.segment_spans.items():
        assert indexed_table.table.column("rerun_segment_id").to_pylist()[start:stop] == [segment_id] * (stop - start)
        assert np.all(np.diff(indexed_table.index_values[start:stop]) >= 0)
    # The `x` values still travel with their rows.
    np.testing.assert_array_equal(
        indexed_table.table.column("x").to_numpy(), indexed_table.index_values.astype(np.float64)
    )


def test_find_segment_boundaries_empty() -> None:
    indexed_table = _find_segment_boundaries(_group_table([], []), "t")

    assert indexed_table.segment_spans == {}
    assert indexed_table.index_values.size == 0


def test_decode_order_is_row_order_not_sampler_order() -> None:
    targets = _targets([("b", 7), ("a", 5), ("b", 3), ("a", 1)])

    order = _decode_order(targets)

    # One group per segment (first-seen order), ascending by index value within a group.
    assert [[(targets[i].segment.segment_id, targets[i].index_value) for i in group] for group in order] == [
        [("b", 3), ("b", 7)],
        [("a", 1), ("a", 5)],
    ]


def test_resolve_decode_requests_resolve_rows_within_their_own_segment() -> None:
    # Index value 5 exists in both segments; each request must resolve to its own segment's row.
    indexed_table = _find_segment_boundaries(_group_table(["a", "a", "b", "b"], [5, 6, 4, 5]), "t")
    targets = _targets([("b", 5), ("a", 5)])
    requests = _resolve_decode_requests(
        [targets[position].fetch_requests["x"] for group in _decode_order(targets) for position in group],
        indexed_table=indexed_table,
    )

    assert [(r.segment_id, r.index_value, r.decode_row_indices) for r in requests] == [
        ("b", 5, (3,)),
        ("a", 5, (0,)),
    ]
    assert all(not r.starts_at_keyframe for r in requests)  # no anchors on these targets


def test_resolve_decode_requests_resolve_a_window_to_a_row_range() -> None:
    indexed_table = _find_segment_boundaries(_group_table(["a", "a", "a", "a"], [0, 1, 2, 3]), "t")
    fetch_request = FieldFetchRequest(
        sample_position=0,
        segment_id="a",
        index_value=1,
        decode_index_range=(1, 3),
        output_index_values=(1, 2, 3),
        fill_latest_at=False,
        requires_contiguous_fetch=False,
        starts_at_keyframe=False,
    )

    (request,) = _resolve_decode_requests(
        [fetch_request],
        indexed_table=indexed_table,
    )

    # The window `(0, 2)` around target 1 covers index values 1..=3, held by rows 1..4.
    assert request.decode_row_indices == (1, 2, 3)
    assert request.output_row_indices == (1, 2, 3)


def test_resolve_decode_requests_maps_explicit_outputs_to_latest_rows() -> None:
    indexed_table = _find_segment_boundaries(_group_table(["a", "a"], [0, 10]), "t")
    fetch_request = FieldFetchRequest(
        sample_position=0,
        segment_id="a",
        index_value=5,
        decode_index_range=(0, 10),
        output_index_values=(0, 5, 10),
        fill_latest_at=True,
        requires_contiguous_fetch=False,
        starts_at_keyframe=False,
    )

    (request,) = _resolve_decode_requests(
        [fetch_request],
        indexed_table=indexed_table,
    )

    assert request.decode_row_indices == (0, 0, 1)
    assert request.output_row_indices == (0, 0, 1)


def test_prepare_block_omits_unresolved_decode_requests() -> None:
    targets = _targets([("a", 0), ("a", 1)])
    fetch_requests = [target.fetch_requests["x"] for target in targets]
    field = Field(path="/x", decode=NumericDecoder())
    indexed = IndexedBlock(
        targets=targets,
        groups=[
            IndexedGroup(
                fields={"x": field},
                fetch_requests={"x": fetch_requests},
                indexed_table=_find_segment_boundaries(
                    _group_table(["a"], [1]).set_column(
                        2,
                        "x",
                        pa.array([[1.0]], type=pa.list_(pa.float64())),
                    ),
                    "t",
                ),
            )
        ],
    )

    prepared = _resolve_decode_requests_in_block(indexed).fields["x"]

    assert len(prepared.requests) == 1
    assert prepared.requests[0].sample_position == 1
    assert prepared.requests[0].output_row_indices == (0,)
    decoded = _decode_field_batch(prepared_field=prepared, num_samples=2, key="x", decoder=NumericDecoder())
    assert decoded[0] is None
    assert decoded[1] is not None
    torch.testing.assert_close(decoded[1], torch.tensor([1.0], dtype=torch.float64))


def test_numeric_decoder_returns_none_for_null_rows_but_preserves_valid_empty_values() -> None:
    targets = _targets([("a", 0), ("a", 1), ("a", 2)])
    field = Field(path="/x", decode=NumericDecoder())
    table = _group_table(["a", "a", "a"], [0, 1, 2]).set_column(
        2,
        "x",
        pa.array([None, [], [1.0]], type=pa.list_(pa.float64())),
    )
    indexed_table = _find_segment_boundaries(table, "t")
    indexed = IndexedBlock(
        targets=targets,
        groups=[
            IndexedGroup(
                fields={"x": field},
                fetch_requests={"x": [target.fetch_requests["x"] for target in targets]},
                indexed_table=indexed_table,
            )
        ],
    )

    prepared = _resolve_decode_requests_in_block(indexed).fields["x"]
    decoded = _decode_field_batch(prepared_field=prepared, num_samples=3, key="x", decoder=NumericDecoder())

    assert [request.sample_position for request in prepared.requests] == [0, 1, 2]
    assert decoded[0] is None
    assert decoded[1] is not None and decoded[1].numel() == 0
    assert decoded[2] is not None
    torch.testing.assert_close(decoded[2], torch.tensor([1.0], dtype=torch.float64))


def test_resolve_decode_requests_keeps_context_rows_separate_from_outputs() -> None:
    indexed_table = _find_segment_boundaries(_group_table(["a", "a", "a", "a"], [0, 1, 2, 3]), "t")
    fetch_request = FieldFetchRequest(
        sample_position=0,
        segment_id="a",
        index_value=3,
        decode_index_range=(0, 3),
        output_index_values=(2, 3),
        fill_latest_at=False,
        requires_contiguous_fetch=True,
        starts_at_keyframe=True,
    )

    (request,) = _resolve_decode_requests(
        [fetch_request],
        indexed_table=indexed_table,
    )

    assert request.decode_row_indices == (0, 1, 2, 3)
    assert request.output_row_indices == (2, 3)


def test_resolve_decode_requests_allows_sparse_contiguous_context() -> None:
    table = _group_table(["a", "a", "a"], [0, 1, 2]).set_column(
        2,
        "x",
        pa.array([[b"first"], None, [b"last"]], type=pa.list_(pa.binary())),
    )
    indexed_table = _find_segment_boundaries(table, "t")
    fetch_request = FieldFetchRequest(
        sample_position=0,
        segment_id="a",
        index_value=2,
        decode_index_range=(0, 2),
        output_index_values=(2,),
        fill_latest_at=False,
        requires_contiguous_fetch=True,
        starts_at_keyframe=True,
    )

    (request,) = _resolve_decode_requests(
        [fetch_request],
        indexed_table=indexed_table,
    )

    assert request.decode_row_indices == (0, 1, 2)
    assert request.output_row_indices == (2,)


def test_resolve_decode_requests_omits_a_segment_with_no_rows() -> None:
    indexed_table = _find_segment_boundaries(_group_table(["a"], [0]), "t")
    targets = _targets([("missing", 0)])

    requests = _resolve_decode_requests(
        [targets[0].fetch_requests["x"]],
        indexed_table=indexed_table,
    )

    assert requests == []
