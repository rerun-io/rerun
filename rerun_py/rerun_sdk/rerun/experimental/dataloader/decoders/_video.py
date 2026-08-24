"""Decoder for compressed video columns, with keyframe-aware random access."""

from __future__ import annotations

from collections import OrderedDict
from typing import TYPE_CHECKING, Any, Generic, Literal, cast, overload

import av
import numpy as np
import torch
from typing_extensions import TypeVar

from rerun._tracing import set_current_span_attributes, with_tracing

from ....components import VideoCodec
from ...video import detect_gop_start, is_annex_b, length_prefixed_to_annex_b
from .._yuv import ColorRange, ColorSpace, Yuv420Frame
from ._arrow import _flatten_blob
from ._base import ColumnDecoder, DecodedValue, DecodeRequest, FieldBatch

if TYPE_CHECKING:
    from collections.abc import Callable, Sequence

    import pyarrow as pa

# AV1 through ``libdav1d`` is faster.
_CODEC_TO_DECODER = {
    "av1": "libdav1d",
    "h264": "h264",
    "h265": "hevc",
    "hevc": "hevc",
}

_CODEC_NAME_ALIASES = {"avc": "h264", "hevc": "h265"}

# Values mirror FFmpeg's AVColorSpace and AVColorRange enums. Matrix
# coefficients without an exact conversion below remain explicit as
# unspecified so callers can provide the correct override.
_COLOR_SPACE_BY_AV_VALUE: dict[int, ColorSpace] = {
    1: "bt709",
    5: "bt601",  # BT.470BG
    6: "bt601",  # SMPTE 170M
    9: "bt2020",  # BT.2020 non-constant luminance
}
_COLOR_RANGE_BY_AV_VALUE: dict[int, ColorRange] = {
    1: "limited",
    2: "full",
}
_DIRECT_YUV420_FORMATS = {"yuv420p", "yuvj420p"}

_OutputFormatT = TypeVar(
    "_OutputFormatT",
    Literal["rgb"],
    Literal["yuv420p"],
    default=Literal["rgb"],
)


def _decoded_frames_are_compatible(frame: DecodedValue, reference: DecodedValue) -> bool:
    """Whether frames may share one stacked bank without losing shape or color metadata."""
    if isinstance(frame, Yuv420Frame) and isinstance(reference, Yuv420Frame):
        return (
            frame.y.shape == reference.y.shape
            and frame.uv.shape == reference.uv.shape
            and frame.color_space == reference.color_space
            and frame.color_range == reference.color_range
        )
    return isinstance(frame, torch.Tensor) and isinstance(reference, torch.Tensor) and frame.shape == reference.shape


def _stack_decoded_frames(frames: list[DecodedValue]) -> DecodedValue:
    """Stack frames emitted by one decoder, whose output format is fixed at construction."""
    first = frames[0]
    if isinstance(first, Yuv420Frame):
        return Yuv420Frame.stack(cast("list[Yuv420Frame]", frames))
    return torch.stack(cast("list[torch.Tensor]", frames))


def _slice_decoded_frame(frame: DecodedValue, index: int | slice) -> DecodedValue:
    if isinstance(frame, Yuv420Frame):
        return Yuv420Frame(
            y=frame.y[index],
            uv=frame.uv[index],
            color_space=frame.color_space,
            color_range=frame.color_range,
        )
    return frame[index]


def _to_video_codec(codec: str) -> VideoCodec | None:
    """
    Map a codec string to [`VideoCodec`][rerun.components.VideoCodec].

    Returns `None` for codecs Rerun doesn't know; every known codec has a
    keyframe detector in `rerun.experimental.video.detect_gop_start`.
    """
    name = _CODEC_NAME_ALIASES.get(codec.lower(), codec.lower())
    return getattr(VideoCodec, name.upper(), None)


def _decoder_name(codec: str) -> str:
    """Resolve public codec names and aliases to the FFmpeg decoder name."""
    name = _CODEC_NAME_ALIASES.get(codec.lower(), codec.lower())
    return _CODEC_TO_DECODER.get(name, name)


class _DecoderSession:
    """An open codec context reused across decode calls that extend the same GOP."""

    __slots__ = ("context", "fed_samples", "frames_emitted", "last_frame")

    def __init__(self, context: av.VideoCodecContext) -> None:
        self.context = context
        self.fed_samples: list[bytes] = []
        self.frames_emitted = 0
        self.last_frame: av.VideoFrame | None = None


def _starts_with(samples: list[bytes], prefix: list[bytes]) -> bool:
    """True if *samples* begins with *prefix*."""
    return len(samples) >= len(prefix) and samples[: len(prefix)] == prefix


def _decode_runs(requests: Sequence[DecodeRequest]) -> list[list[int]]:
    """
    Group decodable request positions into contiguous row runs.

    A run never crosses a segment boundary: rows restart per segment, so two
    segments' windows may not be chained, and each segment's codec session is
    keyed separately. Requests without a prior keyframe are omitted and remain
    unresolved in the decoder output.
    """
    runs: list[list[int]] = []
    for i, request in enumerate(requests):
        if not request.starts_at_keyframe:
            continue
        previous = requests[runs[-1][-1]] if runs else None
        if (
            previous is not None
            and request.segment_id == previous.segment_id
            and request.decode_row_indices[0] <= previous.decode_row_indices[-1] + 1
        ):
            runs[-1].append(i)
        else:
            runs.append([i])
    return runs


def _extract_video_samples(
    column: pa.Array,
    rows: range,
    *,
    video_codec: VideoCodec | None,
) -> tuple[list[bytes], list[int]]:
    """
    Extract the encoded samples of `rows`, plus each non-empty sample's row.

    Empty rows do not contain codec packets. Length-prefixed H.264 packets are
    converted to Annex B before being fed to the raw codec context.
    """
    samples: list[bytes] = []
    sample_rows: list[int] = []
    for row in rows:
        sample_bytes = bytes(_flatten_blob(column, row))
        if not sample_bytes:
            continue
        if video_codec is VideoCodec.H264 and not is_annex_b(sample_bytes):
            sample_bytes = length_prefixed_to_annex_b(sample_bytes)
        samples.append(sample_bytes)
        sample_rows.append(row)
    return samples, sample_rows


class VideoFrameDecoder(ColumnDecoder[DecodedValue], Generic[_OutputFormatT]):
    """
    Compressed video random access via keyframe-aware fetching.

    Anchors the decode window at the prior keyframe by consulting the sibling
    `is_keyframe` component on the `VideoStream` archetype, derived from
    `Field.path` (e.g. `/cam:VideoStream:sample` pairs with
    `/cam:VideoStream:is_keyframe`). The marker is populated by the user or by
    `LazyChunkStream.collect(optimize=…)`, and lives in dedicated chunks
    separate from the video sample, so the lookup is cheap.

    The sibling `is_keyframe` column is required. This makes every decode range
    deterministic rather than relying on an estimated GOP length.

    Samples may be raw H.264 AVC1/AVCC (length-prefixed NAL units) or Annex B;
    the format is detected automatically per sample.

    A batch's requests are grouped by GOP: each GOP's packets are extracted
    and fed through the codec once, and every requested frame is captured as
    it is emitted. A batch (or a later batch) whose window extends an earlier
    one reuses the open codec context and decodes only the new packets.

    With `output_format="rgb"`, a
    [`Field.window`][rerun.experimental.dataloader.Field] returns one frame
    per explicit offset as a `[T, 3, H, W]` tensor. With
    `output_format="yuv420p"`, it returns a [`Yuv420Frame`][rerun.experimental.dataloader.Yuv420Frame] whose Y and UV
    planes remain in YUV form until collation and device transfer.

    Returns `None` when a request's resolved window contains no decodable
    keyframe: the target precedes the entity's first frame in a multi-modal
    segment, or the first row was user-logged `is_keyframe=true` on a sample
    that isn't actually a codec keyframe (run optimize with
    `fix_keyframe=True` to re-derive markers from the encoded samples).
    Consumers must filter these out in their collate function before stacking.
    """

    @overload
    def __init__(
        self: VideoFrameDecoder[Literal["rgb"]],
        *,
        codec: str = "h264",
        max_decoder_sessions: int = 8,
        thread_count: int = 1,
        window_storage: Literal["copy", "view"] = "copy",
        output_format: Literal["rgb"] = "rgb",
    ) -> None: ...

    @overload
    def __init__(
        self: VideoFrameDecoder[Literal["yuv420p"]],
        *,
        codec: str = "h264",
        max_decoder_sessions: int = 8,
        thread_count: int = 1,
        window_storage: Literal["copy", "view"] = "copy",
        output_format: Literal["yuv420p"],
    ) -> None: ...

    def __init__(
        self,
        *,
        codec: str = "h264",
        max_decoder_sessions: int = 8,
        thread_count: int = 1,
        window_storage: Literal["copy", "view"] = "copy",
        output_format: Literal["rgb", "yuv420p"] = "rgb",
    ) -> None:
        """
        Construct a decoder for a compressed video column.

        Parameters
        ----------
        codec:
            Video codec of the encoded samples (e.g. `"h264"`).
        max_decoder_sessions:
            Upper bound on the number of live codec contexts kept in the LRU cache.
            Set to 0 to disable session reuse.
        thread_count:
            FFmpeg decode thread count. `0` leaves thread selection to FFmpeg,
            while `1` requests single-threaded decoding and remains the default
            for a predictable thread budget. For H.264 and H.265, values greater
            than 1 enable frame threading, which decodes several frames concurrently
            but prevents decoder-session reuse because delayed frames must be
            flushed at the end of each decode run.
        window_storage:
            How windowed outputs are materialized. `"copy"` returns independent
            tensors. `"view"` stores each unique decoded frame once per decode
            run and returns views for contiguous windows. Overlapping views share
            storage and must not be mutated in place before collation.
        output_format:
            `"rgb"` returns the existing `[3, H, W]` `uint8` tensor.
            `"yuv420p"` returns a compact [`Yuv420Frame`][rerun.experimental.dataloader.Yuv420Frame] without CPU RGB
            conversion. Stack it with [`Yuv420Frame.stack`][rerun.experimental.dataloader.Yuv420Frame.stack] in an
            application collate function, or use [`Yuv420Collator`][rerun.experimental.dataloader.Yuv420Collator]. Perform
            RGB conversion in the training process after transfer to the GPU.
            Combine it with `window_storage="view"` to decode overlapping
            windows directly into a shared YUV frame bank.

        """
        if max_decoder_sessions < 0:
            raise ValueError(f"max_decoder_sessions must be non-negative, got {max_decoder_sessions}")
        if thread_count < 0:
            raise ValueError(f"thread_count must be non-negative, got {thread_count}")
        if window_storage not in ("copy", "view"):
            raise ValueError(f"window_storage must be 'copy' or 'view', got {window_storage!r}")
        if output_format not in ("rgb", "yuv420p"):
            raise ValueError(f"output_format must be 'rgb' or 'yuv420p', got {output_format!r}")
        self.codec = codec
        self.window_storage = window_storage
        self.output_format = output_format
        # Cached: read per sample in the decode loop.
        self._video_codec = _to_video_codec(codec)
        self._max_decoder_sessions = max_decoder_sessions
        self.thread_count = thread_count

        # LRU of live decode sessions, keyed by `(segment_id, keyframe sample)`.
        self._sessions: OrderedDict[tuple[str, bytes], _DecoderSession] = OrderedDict()
        # Lifetime session-cache stats, surfaced as span attributes on every decode.
        self._cache_hits = 0
        self._cache_misses = 0

    def __repr__(self) -> str:
        options = []
        if self.thread_count != 1:
            options.append(f"thread_count={self.thread_count}")
        if self.window_storage == "view":
            options.append("window_storage='view'")
        if self.output_format == "yuv420p":
            options.append("output_format='yuv420p'")
        suffix = f", {', '.join(options)}" if options else ""
        return f"VideoFrameDecoder(codec={self.codec!r}{suffix})"

    def __getstate__(self) -> dict[str, Any]:
        """Drop the sessions: open codec contexts cannot be pickled. Cache stats restart per process."""
        state = self.__dict__.copy()
        state["_sessions"] = OrderedDict()
        state["_cache_hits"] = 0
        state["_cache_misses"] = 0
        return state

    def prior_keyframe_path(self, field_path: str) -> str | None:
        prefix, sep, _ = field_path.rpartition(":")
        if not sep:
            return None
        return f"{prefix}:is_keyframe"

    @overload
    def decode(
        self: VideoFrameDecoder[Literal["rgb"]],
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> Sequence[torch.Tensor | None]: ...

    @overload
    def decode(
        self: VideoFrameDecoder[Literal["yuv420p"]],
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> Sequence[Yuv420Frame | None]: ...

    @with_tracing("VideoFrameDecoder.decode")
    def decode(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> Sequence[DecodedValue | None]:
        """Decode each request's frame or frame window, feeding every GOP once."""
        out: list[DecodedValue | None] = [None] * len(requests)
        if batch.select is not None:
            # A selector may change row counts, which breaks the row <-> sample
            # mapping the GOP batching relies on; decode per request instead.
            for i, request in enumerate(requests):
                if request.starts_at_keyframe:
                    out[i] = self._decode_selected(batch, request)
            return out

        runs = _decode_runs(requests)
        set_current_span_attributes({
            "rerun.dataloader.video.gop_runs": len(runs),
        })
        for request_positions in runs:
            self._try_decode_run(batch, requests, request_positions, out=out)
        return out

    def _try_decode_run(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
        request_positions: list[int],
        *,
        out: list[DecodedValue | None],
    ) -> None:
        """Decode one GOP run, leaving its outputs unresolved when its encoded data is invalid."""
        try:
            self._decode_run(batch, requests, request_positions, out=out)
        except (av.FFmpegError, ValueError):
            pass

    def _decode_run(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
        request_positions: list[int],
        *,
        out: list[DecodedValue | None],
    ) -> None:
        """Decode one run of requests whose windows form a single contiguous GOP walk."""
        segment_id = requests[request_positions[0]].segment_id

        # 1. Extract one continuous packet sequence for the run.
        # A run's windows chain into one contiguous row window, so its row
        # span is just the union of its requests' spans.
        run_start = min(requests[position].decode_row_indices[0] for position in request_positions)
        run_stop = max(requests[position].decode_row_indices[-1] for position in request_positions) + 1

        # Parallel lists of codec-ready packets and their source Arrow row indices.
        samples, sample_rows = _extract_video_samples(
            batch.column,
            range(run_start, run_stop),
            video_codec=self._video_codec,
        )
        if not samples:
            return
        # Explicit keyframe metadata guarantees that the range starts at a
        # bootstrap packet. Validate the marker when this codec has a detector.
        if self._is_keyframe(samples[0]) is False:
            return

        # 2. Map physical Arrow rows to positions in the non-empty packet sequence.
        # Multiple output slots may resolve to the same latest packet.
        packet_row_indices = np.asarray(sample_rows, dtype=np.int64)
        output_slots_by_packet_position: dict[int, list[tuple[int, int]]] = {}
        frames_by_request_position: dict[int, list[DecodedValue | None]] = {}
        use_views = batch.is_windowed and self.window_storage == "view"
        view_packet_positions_by_request: dict[int, list[int | None]] | None = {} if use_views else None
        for request_position in request_positions:
            request = requests[request_position]
            frames_by_request_position[request_position] = [None] * len(request.output_row_indices)
            if view_packet_positions_by_request is not None:
                view_packet_positions_by_request[request_position] = [None] * len(request.output_row_indices)
            for output_slot, output_row in enumerate(request.output_row_indices):
                packet_position = int(np.searchsorted(packet_row_indices, output_row, side="right")) - 1
                if packet_position < 0 or int(packet_row_indices[packet_position]) < request.decode_row_indices[0]:
                    continue
                output_slots_by_packet_position.setdefault(packet_position, []).append((request_position, output_slot))
                if view_packet_positions_by_request is not None:
                    view_packet_positions_by_request[request_position][output_slot] = packet_position

        if not output_slots_by_packet_position:
            return

        # 3. Walk the GOP once, retaining only frames requested by at least one output slot.
        wanted_packet_positions = sorted(output_slots_by_packet_position)
        packets_to_decode = samples[: wanted_packet_positions[-1] + 1]
        direct_frame_bank: Yuv420Frame | None = None
        direct_frame_bank_compatible = True
        direct_frame_bank_written: set[int] = set()
        capture_frame_fn: Callable[[int, av.VideoFrame], DecodedValue] | None = None
        if use_views and self.output_format == "yuv420p":
            direct_bank_position = {packet_position: i for i, packet_position in enumerate(wanted_packet_positions)}

            def capture_yuv420_frame(packet_position: int, frame: av.VideoFrame) -> Yuv420Frame:
                nonlocal direct_frame_bank, direct_frame_bank_compatible
                planar = self._as_yuv420(frame)
                color_space = self._frame_color_space(frame)
                color_range = self._frame_color_range(frame)
                y_shape: tuple[int, int, int] = (1, planar.planes[0].height, planar.planes[0].width)
                uv_shape: tuple[int, int, int] = (2, planar.planes[1].height, planar.planes[1].width)
                if direct_frame_bank is None:
                    direct_frame_bank = Yuv420Frame(
                        y=torch.empty((len(wanted_packet_positions), *y_shape), dtype=torch.uint8),
                        uv=torch.empty((len(wanted_packet_positions), *uv_shape), dtype=torch.uint8),
                        color_space=color_space,
                        color_range=color_range,
                    )
                bank = direct_frame_bank
                if (
                    tuple(bank.y.shape[1:]) != y_shape
                    or tuple(bank.uv.shape[1:]) != uv_shape
                    or bank.color_space != color_space
                    or bank.color_range != color_range
                ):
                    direct_frame_bank_compatible = False
                    return self._frame_to_yuv420(frame)
                bank_position = direct_bank_position[packet_position]
                tensor = Yuv420Frame(
                    y=bank.y[bank_position],
                    uv=bank.uv[bank_position],
                    color_space=color_space,
                    color_range=color_range,
                )
                self._copy_yuv420_frame(planar, tensor)
                direct_frame_bank_written.add(packet_position)
                return tensor

            capture_frame_fn = capture_yuv420_frame

        decoded_frames_by_packet_position = self._feed_run(
            segment_id,
            packets_to_decode,
            wanted_packet_positions,
            capture_frame=capture_frame_fn,
        )
        if decoded_frames_by_packet_position is None:
            # Delayed stream with multiple wanted frames: emission order didn't
            # map 1:1 to samples, so decode each wanted frame separately from
            # the run's keyframe.
            decoded_frames_by_packet_position = {}
            for packet_position in wanted_packet_positions:
                frame = self._feed_last(
                    segment_id,
                    samples[: packet_position + 1],
                    capture_frame=capture_frame_fn,
                )
                if frame is not None:
                    decoded_frames_by_packet_position[packet_position] = frame

        if view_packet_positions_by_request is not None and self._try_emit_view_windows(
            packet_positions_by_request=view_packet_positions_by_request,
            wanted_packet_positions=wanted_packet_positions,
            decoded_frames_by_packet_position=decoded_frames_by_packet_position,
            direct_frame_bank=direct_frame_bank,
            direct_frame_bank_compatible=direct_frame_bank_compatible,
            direct_frame_bank_written=direct_frame_bank_written,
            out=out,
        ):
            return

        # 4. Scatter captured frames back into each request's ordered output slots.
        for packet_position, tensor in decoded_frames_by_packet_position.items():
            for request_position, output_slot in output_slots_by_packet_position[packet_position]:
                frames_by_request_position[request_position][output_slot] = (
                    tensor if batch.is_windowed else tensor.clone()
                )

        for request_position, frames in frames_by_request_position.items():
            if any(frame is None for frame in frames):
                continue
            resolved = cast("list[DecodedValue]", frames)

            if batch.is_windowed:
                if any(not _decoded_frames_are_compatible(frame, resolved[0]) for frame in resolved[1:]):
                    continue
                out[request_position] = _stack_decoded_frames(resolved)
            else:
                out[request_position] = resolved[0]

    @staticmethod
    def _try_emit_view_windows(
        *,
        packet_positions_by_request: dict[int, list[int | None]],
        wanted_packet_positions: list[int],
        decoded_frames_by_packet_position: dict[int, DecodedValue],
        direct_frame_bank: Yuv420Frame | None,
        direct_frame_bank_compatible: bool,
        direct_frame_bank_written: set[int],
        out: list[DecodedValue | None],
    ) -> bool:
        """Populate view-mode outputs from one shared frame bank when at least one window is contiguous."""
        decoded_packet_positions = [
            position for position in wanted_packet_positions if position in decoded_frames_by_packet_position
        ]
        bank_index_by_packet_position = {
            packet_position: bank_index for bank_index, packet_position in enumerate(decoded_packet_positions)
        }

        # Translate each fully decoded request from packet positions into frame-bank indices.
        bank_indices_by_request: dict[int, list[int]] = {}
        view_slices_by_request: dict[int, slice] = {}
        for request_position, packet_positions in packet_positions_by_request.items():
            if any(position is None or position not in bank_index_by_packet_position for position in packet_positions):
                continue
            resolved_packet_positions = cast("list[int]", packet_positions)
            bank_indices = [bank_index_by_packet_position[position] for position in resolved_packet_positions]
            bank_indices_by_request[request_position] = bank_indices

            start = bank_indices[0]
            if all(bank_index == start + offset for offset, bank_index in enumerate(bank_indices)):
                view_slices_by_request[request_position] = slice(start, start + len(bank_indices))

        # Building a shared bank only pays off when at least one request can use a slice view.
        if not view_slices_by_request:
            return False

        bank_frames = [decoded_frames_by_packet_position[position] for position in decoded_packet_positions]
        if not bank_frames or not all(
            _decoded_frames_are_compatible(frame, bank_frames[0]) for frame in bank_frames[1:]
        ):
            return False

        # The YUV path may have decoded directly into the correctly ordered bank.
        direct_bank_ready = (
            direct_frame_bank is not None
            and direct_frame_bank_compatible
            and decoded_packet_positions == wanted_packet_positions
            and len(direct_frame_bank_written) == len(wanted_packet_positions)
        )
        if direct_bank_ready:
            assert direct_frame_bank is not None
            frame_bank: DecodedValue = direct_frame_bank
        else:
            frame_bank = _stack_decoded_frames(bank_frames)

        for request_position, bank_indices in bank_indices_by_request.items():
            view_slice = view_slices_by_request.get(request_position)
            if view_slice is not None:
                out[request_position] = _slice_decoded_frame(frame_bank, view_slice)
            else:
                out[request_position] = _stack_decoded_frames([
                    _slice_decoded_frame(frame_bank, bank_index) for bank_index in bank_indices
                ])
        return True

    def _feed_run(
        self,
        segment_id: str,
        feed: list[bytes],
        wanted: list[int],
        *,
        capture_frame: Callable[[int, av.VideoFrame], DecodedValue] | None = None,
    ) -> dict[int, DecodedValue] | None:
        """
        Feed *feed* through a (possibly cached) session, capturing the frames at *wanted* positions.

        `wanted` holds ascending sample positions into `feed`, ending at
        `len(feed) - 1`. For delay-free streams (one frame out per packet in)
        the codec context is kept open, and a later call whose window extends
        this one decodes only the new packets.

        Returns `None` when the stream turns out to be delayed (B-frames or
        pipelining) and more than the final frame was wanted: emitted frames
        then don't map 1:1 to samples, and the caller must decode per request.
        """
        session_key = (segment_id, feed[0])
        session = self._sessions.pop(session_key, None)
        if (
            session is not None
            and _starts_with(feed, session.fed_samples)
            # A frame before the session's last emitted one was already
            # discarded; only a fresh context can replay it.
            and wanted[0] >= session.frames_emitted - 1
        ):
            self._cache_hits += 1
        else:
            self._cache_misses += 1
            session = _DecoderSession(self._create_context())

        total = self._cache_hits + self._cache_misses
        set_current_span_attributes({
            "rerun.dataloader.video.session_cache_hits": self._cache_hits,
            "rerun.dataloader.video.session_cache_misses": self._cache_misses,
            "rerun.dataloader.video.session_cache_hit_rate": self._cache_hits / total,
            "rerun.dataloader.video.session_cache_miss_rate": self._cache_misses / total,
            "rerun.dataloader.video.window_samples": len(feed),
            "rerun.dataloader.video.packets_fed": len(feed) - len(session.fed_samples),
            "rerun.dataloader.video.frames_wanted": len(wanted),
        })

        frame_capture: Callable[[int, av.VideoFrame], DecodedValue]
        if capture_frame is None:

            def default_capture(_position: int, frame: av.VideoFrame) -> DecodedValue:
                return self._frame_to_output(frame)

            frame_capture = default_capture
        else:
            frame_capture = capture_frame

        captured: dict[int, DecodedValue] = {}
        wanted_set = set(wanted)
        saw_b_frame = False

        def receive_frame(frame: av.VideoFrame) -> None:
            nonlocal saw_b_frame
            pos = session.frames_emitted
            session.frames_emitted += 1
            session.last_frame = frame
            saw_b_frame |= frame.pict_type == av.video.frame.PictureType.B
            if pos in wanted_set:
                captured[pos] = frame_capture(pos, frame)

        if session.last_frame is not None and session.frames_emitted - 1 in wanted_set:
            # The session's last emitted frame is still at hand (repeated target).
            position = session.frames_emitted - 1
            captured[position] = frame_capture(position, session.last_frame)

        # The session stays popped while feeding, so a raising packet can't
        # leave a corrupt context behind.
        for sample in feed[len(session.fed_samples) :]:
            for frame in session.context.decode(av.Packet(sample)):
                receive_frame(frame)
        session.fed_samples = feed

        uses_frame_threading = self.thread_count > 1 and _decoder_name(self.codec) in {"h264", "hevc"}
        if session.frames_emitted == len(feed) and not uses_frame_threading:
            # Delay-free stream: emission order matched sample order, so every
            # captured frame is correct. Keep the context open for extension.
            self._sessions[session_key] = session
            evicted = 0
            while len(self._sessions) > self._max_decoder_sessions:
                self._sessions.popitem(last=False)
                evicted += 1
            set_current_span_attributes({
                "rerun.dataloader.video.session_kept": True,
                "rerun.dataloader.video.sessions_evicted": evicted,
                "rerun.dataloader.video.live_sessions": len(self._sessions),
            })
            return captured

        # Frame threading pipelines several packets before emitting their
        # corresponding frames. Drain the pipeline once and assign those frames
        # the next sequential positions. Rerun VideoStream samples require
        # DTS == PTS, so this mapping is valid unless the stream contains
        # unsupported B-frames.
        set_current_span_attributes({
            "rerun.dataloader.video.session_kept": False,
            "rerun.dataloader.video.live_sessions": len(self._sessions),
        })
        for frame in session.context.decode(None):
            receive_frame(frame)

        if session.frames_emitted == len(feed) and not saw_b_frame:
            return captured

        # A B-frame stream has decode order != presentation order, while raw
        # packets carry no timestamps here. Preserve the existing conservative
        # behavior: replay through the requested position and take the final
        # displayed frame.
        if wanted != [len(feed) - 1]:
            return None
        target_frame = session.last_frame
        if target_frame is None:
            return {}
        position = len(feed) - 1
        return {position: frame_capture(position, target_frame)}

    def _feed_last(
        self,
        segment_id: str,
        feed: list[bytes],
        *,
        capture_frame: Callable[[int, av.VideoFrame], DecodedValue] | None = None,
    ) -> DecodedValue | None:
        """
        Feed *feed* through a (possibly cached) session and return its final frame, if any.

        Unlike a multi-frame `_feed_run`, the final frame is the last frame the
        codec emits, so on a delayed stream the flush can serve it directly.
        """
        captured = self._feed_run(segment_id, feed, [len(feed) - 1], capture_frame=capture_frame)
        return None if captured is None else captured.get(len(feed) - 1)

    def _decode_selected(self, batch: FieldBatch, request: DecodeRequest) -> DecodedValue | None:
        """Decode one request whose context went through a row-preserving selector."""
        decode_rows = batch.take_decode_rows(request)
        if len(decode_rows) != len(request.decode_row_indices):
            raise ValueError(
                f"Selector returned {len(decode_rows)} rows for {len(request.decode_row_indices)} video context rows; "
                "video decoding requires a selector that preserves row count"
            )
        row_positions = {row: position for position, row in enumerate(request.decode_row_indices)}
        output_row_indices = tuple(row_positions[row] for row in request.output_row_indices)
        adjusted = DecodeRequest(
            sample_position=request.sample_position,
            segment_id=request.segment_id,
            index_value=request.index_value,
            decode_row_indices=tuple(range(len(decode_rows))),
            output_row_indices=output_row_indices,
            starts_at_keyframe=request.starts_at_keyframe,
        )
        out: list[DecodedValue | None] = [None]
        self._try_decode_run(FieldBatch(column=decode_rows, is_windowed=batch.is_windowed), [adjusted], [0], out=out)
        return out[0]

    def _is_keyframe(self, sample: bytes) -> bool | None:
        """Whether *sample* can boot the decoder, or `None` if we have no detector for this codec."""
        if self._video_codec is None:
            return None
        try:
            return detect_gop_start(sample, self._video_codec)
        except ValueError:
            # Malformed GOP-start candidate (e.g. unparsable SPS): can't bootstrap from it.
            return False

    def _create_context(self) -> av.VideoCodecContext:
        """A fresh raw-packet CodecContext (no container)."""
        decoder_name = _decoder_name(self.codec)
        context = cast("av.VideoCodecContext", av.CodecContext.create(decoder_name, "r"))
        context.thread_count = self.thread_count
        if self.thread_count > 1 and decoder_name in {"h264", "hevc"}:
            context.thread_type = "FRAME"
        if decoder_name == "libdav1d":
            # dav1d delays output for pipelining by default; the session fast
            # path needs one frame out per packet in.
            context.options = {"max_frame_delay": "1"}
        return context

    def _frame_to_output(self, frame: av.VideoFrame) -> DecodedValue:
        """Convert a decoded frame to the configured public representation."""
        if self.output_format == "yuv420p":
            return self._frame_to_yuv420(frame)
        return self._frame_to_tensor(frame)

    def _frame_to_tensor(self, frame: av.VideoFrame) -> torch.Tensor:
        """Convert a PyAV VideoFrame to a `(3, H, W)` uint8 tensor."""
        arr = frame.to_ndarray(format="rgb24")
        return torch.from_numpy(arr).permute(2, 0, 1)

    def _frame_to_yuv420(self, frame: av.VideoFrame) -> Yuv420Frame:
        """Copy a decoded frame's YUV420 planes without performing RGB conversion."""
        planar = self._as_yuv420(frame)
        out = Yuv420Frame(
            y=torch.empty((1, planar.planes[0].height, planar.planes[0].width), dtype=torch.uint8),
            uv=torch.empty((2, planar.planes[1].height, planar.planes[1].width), dtype=torch.uint8),
            color_space=self._frame_color_space(frame),
            color_range=self._frame_color_range(frame),
        )
        self._copy_yuv420_frame(planar, out)
        return out

    @staticmethod
    def _frame_color_space(frame: av.VideoFrame) -> ColorSpace:
        """Map FFmpeg matrix-coefficient metadata to the conversion helper's supported spaces."""
        return _COLOR_SPACE_BY_AV_VALUE.get(int(frame.colorspace), "unspecified")

    @staticmethod
    def _frame_color_range(frame: av.VideoFrame) -> ColorRange:
        """Map FFmpeg range metadata to full, limited, or explicitly unspecified."""
        color_range = _COLOR_RANGE_BY_AV_VALUE.get(int(frame.color_range))
        if color_range is not None:
            return color_range
        # The deprecated yuvj family encodes full range in the pixel format
        # when AVFrame.color_range itself is unspecified.
        if frame.format.name.startswith("yuvj"):
            return "full"
        return "unspecified"

    @staticmethod
    def _as_yuv420(frame: av.VideoFrame) -> av.VideoFrame:
        """Return directly copyable 8-bit planar 4:2:0, preserving yuvj full-range samples."""
        if frame.format.name in _DIRECT_YUV420_FORMATS:
            return frame
        return frame.reformat(format="yuv420p")

    @staticmethod
    def _copy_yuv420_frame(frame: av.VideoFrame, out: Yuv420Frame) -> None:
        """Copy visible Y, U, and V plane rows while excluding FFmpeg's line padding."""
        destinations = (out.y[0], out.uv[0], out.uv[1])
        for plane, destination in zip(frame.planes, destinations, strict=True):
            source = torch.frombuffer(plane, dtype=torch.uint8).as_strided(
                (plane.height, plane.width),
                (plane.line_size, 1),
            )
            destination.copy_(source)
