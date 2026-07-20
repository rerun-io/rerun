from __future__ import annotations

import io
from abc import ABC, abstractmethod
from collections import OrderedDict
from typing import Any, cast

import av
import numpy as np
import pyarrow as pa
import torch
from PIL import Image
from torchvision.transforms.functional import pil_to_tensor  # type: ignore[import-untyped]

from rerun._tracing import with_tracing

from ._sample_index import _ns_to_datetime64, _ns_to_timedelta64

# AV1 through ``libdav1d`` is faster.
_CODEC_TO_DECODER = {
    "av1": "libdav1d",
    "h264": "h264",
    "h265": "hevc",
    "hevc": "hevc",
}

_ANNEX_B_START_CODE = b"\x00\x00\x00\x01"
_ANNEX_B_START_CODE_SHORT = b"\x00\x00\x01"


class ColumnDecoder(ABC):
    """
    Base class for column decoders.

    Subclasses convert raw Arrow data into tensors. Stateless decoders
    (images, scalars) only need to implement [`decode`][rerun.experimental.dataloader.ColumnDecoder.decode].
    Context-aware decoders (compressed video) should also override
    [`context_range`][rerun.experimental.dataloader.ColumnDecoder.context_range] so the prefetcher fetches surrounding data.
    """

    @abstractmethod
    def decode(
        self,
        raw: pa.ChunkedArray,
        index_value: int | np.datetime64 | np.timedelta64,
        segment_id: str,
    ) -> torch.Tensor | None:
        """Decode *raw* Arrow data into a tensor, or return `None` to signal data missing."""
        ...

    def context_range(
        self,
        index_value: int | np.datetime64 | np.timedelta64,
    ) -> tuple[int | np.datetime64 | np.timedelta64, int | np.datetime64 | np.timedelta64] | None:
        """
        Extra index-value range needed to decode *index_value*.

        Returns `(start, end)` inclusive, or `None` when only the
        exact index value is required (the default).
        """
        del index_value
        return None

    def prior_keyframe_path(self, field_path: str) -> str | None:
        """
        Sibling column whose non-null rows mark a re-entrant keyframe, or `None`.

        Override on decoders that need the prefetch window anchored at the prior
        keyframe (compressed video). Default returns `None`.
        """
        del field_path
        return None

    @property
    def fill_latest_at(self) -> bool:
        """
        Whether this column's prefetch read latest-at-fills empty grid slots.

        `True` for stateless columns (images, scalars): each grid slot wants the
        most recent value snapped from the real rows. Compressed video keeps it
        `True` too (consecutive duplicates from a dense grid are dropped at
        decode time), but a decoder reading frame-indexed data where the grid
        lands 1:1 on real samples can override to `False` for exact, fill-free
        packet reads. The read is partitioned by this flag so it stays a global
        query argument per group rather than a per-column one.
        """
        return True

    def __repr__(self) -> str:
        return f"{type(self).__name__}()"


class ImageDecoder(ColumnDecoder):
    """Decode a single encoded-image blob (JPEG/PNG) to a `[C, H, W]` uint8 tensor."""

    @with_tracing("ImageDecoder.decode")
    def decode(
        self,
        raw: pa.ChunkedArray,
        index_value: int | np.datetime64 | np.timedelta64,
        segment_id: str,
    ) -> torch.Tensor:
        del index_value, segment_id
        combined = raw.combine_chunks()
        blob_bytes = bytes(_flatten_blob(combined, 0))
        image = Image.open(io.BytesIO(blob_bytes))
        return pil_to_tensor(image)  # type: ignore[no-any-return]


class NumericDecoder(ColumnDecoder):
    """Decode Arrow numeric / list-of-numeric columns to a tensor."""

    @with_tracing("NumericDecoder.decode")
    def decode(
        self,
        raw: pa.ChunkedArray,
        index_value: int | np.datetime64 | np.timedelta64,
        segment_id: str,
    ) -> torch.Tensor:
        del index_value, segment_id
        return torch.as_tensor(_unwrap_to_numpy(raw.combine_chunks()))


def _unwrap_to_numpy(arr: pa.Array) -> np.ndarray:
    """
    Recursively unwrap nested Arrow list types to a numpy array.

    Handles `list<double>`, `list<list<double>>`,
    `fixed_size_list<float>`, and plain numeric arrays.
    """
    if _is_list_type(arr.type):
        inner = arr.values
        if _is_list_type(inner.type):
            return _unwrap_to_numpy(inner)
        arr = inner

    # Torch requires writeable arrays; a zero-copy view into the Arrow buffer is not.
    numpy_array = arr.to_numpy(zero_copy_only=False)
    if not numpy_array.flags.writeable:
        numpy_array = numpy_array.copy()
    return numpy_array  # type: ignore[no-any-return]


def _is_list_type(t: pa.DataType) -> bool:
    return bool(pa.types.is_list(t) or pa.types.is_large_list(t) or pa.types.is_fixed_size_list(t))


def _flatten_blob(arr: pa.Array, row: int) -> np.ndarray:
    """Extract row *row* bytes from a `list<list<uint8>>` or `list<binary | large_binary>` array."""
    outer_offsets = arr.offsets.to_numpy()
    lo, hi = int(outer_offsets[row]), int(outer_offsets[row + 1])
    inner = arr.values.slice(lo, hi - lo)

    if _is_list_type(inner.type):
        # `flatten()` respects the slice's offsets, unlike `.values`.
        return inner.flatten().to_numpy(zero_copy_only=False)  # type: ignore[no-any-return]

    # BinaryArray rows are contiguous in the values buffer; slice via offsets.
    offset_dtype = np.int64 if pa.types.is_large_binary(inner.type) else np.int32
    offsets = np.frombuffer(inner.buffers()[1], dtype=offset_dtype)
    start = int(offsets[inner.offset])
    end = int(offsets[inner.offset + len(inner)])
    return np.frombuffer(inner.buffers()[2], dtype=np.uint8, offset=start, count=end - start)


def _avcc_to_annex_b(data: bytes, nal_length_size: int = 4) -> bytes:
    """Convert AVCC/AVC1 (length-prefixed) NAL units to Annex B (start-code-prefixed)."""
    result = bytearray()
    pos = 0
    while pos + nal_length_size <= len(data):
        nal_length = int.from_bytes(data[pos : pos + nal_length_size], "big")
        pos += nal_length_size
        if nal_length <= 0 or pos + nal_length > len(data):
            break
        result.extend(_ANNEX_B_START_CODE)
        result.extend(data[pos : pos + nal_length])
        pos += nal_length
    return bytes(result)


def _is_annex_b(data: bytes) -> bool:
    """Check if data starts with an Annex B start code."""
    return data[:4] == _ANNEX_B_START_CODE or data[:3] == _ANNEX_B_START_CODE_SHORT


def _is_av1_keyframe_packet(sample: bytes) -> bool:
    """
    Heuristic: True if *sample*'s first OBU is `OBU_SEQUENCE_HEADER` (type 1) or `OBU_TEMPORAL_DELIMITER` (type 2).

    libdav1d rejects a non-keyframe as the first packet, so we use this to skip leading non-keyframe
    samples until something keyframe-like appears.

    Assumes the upstream encoder emits TDs only at random-access points,
    for streams where every TU starts with a TD this check is a no-op and the first sample is always treated as a keyframe.
    """
    if not sample:
        return False
    obu_type = (sample[0] >> 3) & 0xF
    return obu_type in (1, 2)


def _h264_annex_b_has_idr(sample: bytes) -> bool:
    """True if *sample* (Annex-B H.264) contains an IDR slice NAL (type 5)."""
    pos = 0
    while True:
        idx = sample.find(b"\x00\x00\x01", pos)
        if idx < 0 or idx + 3 >= len(sample):
            return False
        if (sample[idx + 3] & 0x1F) == 5:
            return True
        pos = idx + 3


def _hevc_annex_b_has_irap(sample: bytes) -> bool:
    """True if *sample* (Annex-B HEVC) contains an IRAP NAL (type 16-23)."""
    pos = 0
    while True:
        idx = sample.find(b"\x00\x00\x01", pos)
        if idx < 0 or idx + 3 >= len(sample):
            return False
        nal_type = (sample[idx + 3] >> 1) & 0x3F
        if 16 <= nal_type <= 23:
            return True
        pos = idx + 3


class _DecoderSession:
    """An open codec context reused across `decode` calls that extend the same GOP."""

    __slots__ = ("context", "fed_samples", "frames_emitted", "last_frame")

    def __init__(self, context: av.VideoCodecContext) -> None:
        self.context = context
        self.fed_samples: list[bytes] = []
        self.frames_emitted = 0
        self.last_frame: av.VideoFrame | None = None


def _starts_with(samples: list[bytes], prefix: list[bytes]) -> bool:
    """True if *samples* begins with *prefix*."""
    return len(samples) >= len(prefix) and samples[: len(prefix)] == prefix


class VideoFrameDecoder(ColumnDecoder):
    """
    Compressed video random access via context-aware fetching.

    Anchors the decode window at the prior keyframe by consulting the sibling
    `is_keyframe` component on the `VideoStream` archetype, derived from
    `Field.path` (e.g. `/cam:VideoStream:sample` pairs with
    `/cam:VideoStream:is_keyframe`). The marker is populated by the user or by
    `LazyChunkStream.collect(optimize=…)`, and lives in dedicated chunks
    separate from the video sample, so the lookup is cheap.

    When the column is missing from the schema, or has no row at or before
    the target, the decoder falls back to a fixed-size window: the previous
    `keyframe_interval` samples (counted directly for integer indices,
    converted to `keyframe_interval / fps_estimate` seconds for timestamp
    indices). `keyframe_interval` must be at least the actual GOP length, and
    for timestamp indices `fps_estimate` must be close to the true frame rate.

    Samples may be raw H.264 AVC1/AVCC (length-prefixed NAL units) or Annex B;
    the format is detected automatically per sample.

    A call whose window extends the previous call's (same GOP) reuses an open
    codec context and decodes only the new packets.

    Returns `None` when the resolved window contains no decodable keyframe:
    the target precedes the entity's first frame in a multi-modal segment,
    the fallback `keyframe_interval` under-estimates the true GOP length, or
    the anchored row was user-logged `is_keyframe=true` on a sample that
    isn't actually a codec keyframe (run optimize with `fix_keyframe=True` to
    re-derive markers from the encoded samples). Consumers must filter these
    out in their collate function before stacking.
    """

    def __init__(
        self,
        *,
        keyframe_interval: int = 30,
        fps_estimate: float = 30.0,
        codec: str = "h264",
        max_decoder_sessions: int = 8,
    ) -> None:
        self.codec = codec
        self._decoder_name = _CODEC_TO_DECODER.get(codec, codec)
        self._keyframe_interval = keyframe_interval
        self._keyframe_duration_ns = int(keyframe_interval / fps_estimate * 1e9)
        self._max_decoder_sessions = max_decoder_sessions
        # LRU of live decode sessions, keyed by `(segment_id, keyframe sample)`.
        self._sessions: OrderedDict[tuple[str, bytes], _DecoderSession] = OrderedDict()

    def __repr__(self) -> str:
        return f"VideoFrameDecoder(codec={self.codec!r})"

    def __getstate__(self) -> dict[str, Any]:
        """Drop the sessions: open codec contexts cannot be pickled."""
        state = self.__dict__.copy()
        state["_sessions"] = OrderedDict()
        return state

    def prior_keyframe_path(self, field_path: str) -> str | None:
        prefix, sep, _ = field_path.rpartition(":")
        if not sep:
            return None
        return f"{prefix}:is_keyframe"

    def context_range(
        self,
        index_value: int | np.datetime64 | np.timedelta64,
    ) -> tuple[int | np.datetime64 | np.timedelta64, int | np.datetime64 | np.timedelta64] | None:
        """Need frames from estimated keyframe position to target."""
        if isinstance(index_value, np.datetime64):
            iv = int(np.int64(index_value))
            return (_ns_to_datetime64(iv - self._keyframe_duration_ns), index_value)
        if isinstance(index_value, np.timedelta64):
            iv = int(np.int64(index_value))
            return (_ns_to_timedelta64(iv - self._keyframe_duration_ns), index_value)
        iv = int(index_value)
        return (max(0, iv - self._keyframe_interval), iv)

    @with_tracing("VideoFrameDecoder.decode")
    def decode(
        self,
        raw: pa.ChunkedArray,
        index_value: int | np.datetime64 | np.timedelta64,
        segment_id: str,
    ) -> torch.Tensor | None:
        """Decode the target frame from the context samples in *raw*, or `None` if no keyframe is available."""
        return self._decode_to_target(raw, index_value, segment_id)

    def _decode_to_target(
        self,
        raw_context: pa.ChunkedArray,
        target_idx: int | np.datetime64 | np.timedelta64,
        segment_id: str,
    ) -> torch.Tensor | None:
        """
        Decode context through *target_idx* and return the final frame.

        `context_range` ends exactly at *target_idx*, so the target is always
        the last decoded frame. For delay-free streams (one frame out per
        packet in) the codec context is kept open, and a later call whose
        window extends this one decodes only the new packets.
        """
        combined = raw_context.combine_chunks()
        num_rows = len(combined)

        samples: list[bytes] = []
        for i in range(num_rows):
            sample_bytes = bytes(_flatten_blob(combined, i))
            if not sample_bytes:
                continue
            if self.codec == "h264" and not _is_annex_b(sample_bytes):
                sample_bytes = _avcc_to_annex_b(sample_bytes)
            # `fill_latest_at` repeats the previous frame's bytes for grid slots
            # with no source frame, so the window can hold consecutive duplicate
            # samples. Re-feeding a duplicate packet corrupts the decoder's
            # reference state, so skip them.
            # TODO(RR-4751): we should measure whether we can optimize this by doing precise queries when `VideoStream::is_keyframe` is present.
            if samples and sample_bytes == samples[-1]:
                continue
            samples.append(sample_bytes)

        # No bootstrap context: target precedes the first keyframe in the
        # prefetched range. See class docstring.
        if not self._has_keyframe(samples):
            return None

        # For codecs we recognize, drop leading non-keyframe samples so the decoder sees a
        # bootstrap packet first (libdav1d rejects a non-keyframe outright;
        # H.264/HEVC need SPS/PPS, plus VPS for HEVC, before any non-IDR/IRAP slice).
        # For codecs without a detector, `_is_keyframe` returns None and the loop is a no-op.
        drop = 0
        while drop < len(samples):
            is_keyframe = self._is_keyframe(samples[drop])
            if is_keyframe is None or is_keyframe:
                break
            drop += 1
        samples = samples[drop:]

        # `samples[0]` is the window's keyframe, distinguishing GOPs within a segment.
        session_key = (segment_id, samples[0])
        session = self._sessions.pop(session_key, None)
        if session is None or not _starts_with(samples, session.fed_samples):
            session = _DecoderSession(self._create_context())

        # The session stays popped while feeding, so a raising packet can't
        # leave a corrupt context behind.
        for sample in samples[len(session.fed_samples) :]:
            for frame in session.context.decode(av.Packet(sample)):
                session.frames_emitted += 1
                session.last_frame = frame
        session.fed_samples = samples

        if session.frames_emitted == len(samples) and session.last_frame is not None:
            # Delay-free stream: the last emitted frame is the target; keep the context open.
            self._sessions[session_key] = session
            while len(self._sessions) > self._max_decoder_sessions:
                self._sessions.popitem(last=False)
            return self._frame_to_tensor(session.last_frame)

        # Delayed frames (B-frames or pipelining): flush. A flushed context
        # cannot be re-fed, so no session is kept.
        target_frame = session.last_frame
        for frame in session.context.decode(None):
            target_frame = frame

        if target_frame is None:
            raise RuntimeError(
                f"Failed to decode target frame {target_idx} for segment {segment_id}: "
                f"{len(samples)} context samples included a keyframe but the decoder "
                "produced no frame."
            )

        return self._frame_to_tensor(target_frame)

    def _is_keyframe(self, sample: bytes) -> bool | None:
        """Whether *sample* can boot the decoder, or `None` if we have no detector for this codec."""
        if self.codec == "av1":
            return _is_av1_keyframe_packet(sample)
        if self.codec == "h264":
            return _h264_annex_b_has_idr(sample)
        if self.codec in ("h265", "hevc"):
            return _hevc_annex_b_has_irap(sample)
        return None

    def _has_keyframe(self, samples: list[bytes]) -> bool:
        """True if *samples* has a known-codec keyframe, or this codec has no detector (then we trust the decoder)."""
        for sample in samples:
            is_keyframe = self._is_keyframe(sample)
            if is_keyframe is None:
                return True
            if is_keyframe:
                return True
        return False

    def _create_context(self) -> av.VideoCodecContext:
        """A fresh raw-packet CodecContext (no container)."""
        context = cast("av.VideoCodecContext", av.CodecContext.create(self._decoder_name, "r"))
        if self._decoder_name == "libdav1d":
            # dav1d delays output for pipelining by default; the session fast
            # path needs one frame out per packet in.
            context.options = {"max_frame_delay": "1"}
        return context

    def _frame_to_tensor(self, frame: av.VideoFrame) -> torch.Tensor:
        """Convert a PyAV VideoFrame to a `(3, H, W)` uint8 tensor."""
        arr = frame.to_ndarray(format="rgb24")
        return torch.from_numpy(arr).permute(2, 0, 1)
