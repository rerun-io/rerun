"""Decoder for compressed video columns, with keyframe-aware random access."""

from __future__ import annotations

from collections import OrderedDict
from typing import TYPE_CHECKING, Any, cast

import av
import numpy as np
import torch

from rerun._tracing import set_current_span_attributes, with_tracing

from ....components import VideoCodec
from ...video import detect_gop_start, is_annex_b, length_prefixed_to_annex_b
from ._arrow import _flatten_blob
from ._base import ColumnDecoder, DecodeRequest, FieldBatch

if TYPE_CHECKING:
    from collections.abc import Sequence

    import pyarrow as pa

# AV1 through ``libdav1d`` is faster.
_CODEC_TO_DECODER = {
    "av1": "libdav1d",
    "h264": "h264",
    "h265": "hevc",
    "hevc": "hevc",
}

_CODEC_NAME_ALIASES = {"avc": "h264", "hevc": "h265"}


def _to_video_codec(codec: str) -> VideoCodec | None:
    """
    Map a codec string to [`VideoCodec`][rerun.components.VideoCodec].

    Returns `None` for codecs Rerun doesn't know; every known codec has a
    keyframe detector in `rerun.experimental.video.detect_gop_start`.
    """
    name = _CODEC_NAME_ALIASES.get(codec.lower(), codec.lower())
    return getattr(VideoCodec, name.upper(), None)


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


class VideoFrameDecoder(ColumnDecoder):
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

    A [`Field.window`][rerun.experimental.dataloader.Field] returns one frame
    per explicit offset as a `[T, 3, H, W]` tensor.

    Returns `None` when a request's resolved window contains no decodable
    keyframe: the target precedes the entity's first frame in a multi-modal
    segment, or the first row was user-logged `is_keyframe=true` on a sample
    that isn't actually a codec keyframe (run optimize with
    `fix_keyframe=True` to re-derive markers from the encoded samples).
    Consumers must filter these out in their collate function before stacking.
    """

    def __init__(
        self,
        *,
        codec: str = "h264",
        max_decoder_sessions: int = 8,
        thread_count: int = 1,
    ) -> None:
        """
        Construct a decoder for a compressed video column.

        Parameters
        ----------
        codec:
            Video codec of the encoded samples (e.g. `"h264"`).
        max_decoder_sessions:
            Upper bound on the number of live codec contexts kept in the LRU cache.
        thread_count:
            ffmpeg decode thread count. Usually 1 for low resolution, larger for
            large resolutions; 1 is preferred over auto, so we do not propose auto.

        """
        self.codec = codec
        # Cached: read per sample in the decode loop.
        self._video_codec = _to_video_codec(codec)
        self._max_decoder_sessions = max_decoder_sessions
        # TODO(guillaume): expose `thread_count` as a user-facing parameter
        # if some customers do want to decode large images.
        self.thread_count = thread_count

        # LRU of live decode sessions, keyed by `(segment_id, keyframe sample)`.
        self._sessions: OrderedDict[tuple[str, bytes], _DecoderSession] = OrderedDict()
        # Lifetime session-cache stats, surfaced as span attributes on every decode.
        self._cache_hits = 0
        self._cache_misses = 0

    def __repr__(self) -> str:
        return f"VideoFrameDecoder(codec={self.codec!r})"

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

    @with_tracing("VideoFrameDecoder.decode")
    def decode(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> list[torch.Tensor | None]:
        """Decode each request's frame or frame window, feeding every GOP once."""
        out: list[torch.Tensor | None] = [None] * len(requests)
        if batch.select is not None:
            # A selector may change row counts, which breaks the row <-> sample
            # mapping the GOP batching relies on; decode per request instead.
            for i, request in enumerate(requests):
                if request.starts_at_keyframe:
                    out[i] = self._decode_selected(batch, request)
            return out

        runs = _decode_runs(requests)
        set_current_span_attributes({
            "rerun.dataloader.video.num_requests": len(requests),
            "rerun.dataloader.video.num_segments": len({request.segment_id for request in requests}),
            "rerun.dataloader.video.gop_runs": len(runs),
        })
        for request_positions in runs:
            self._decode_run(batch, requests, request_positions, out=out)
        return out

    def _decode_run(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
        request_positions: list[int],
        *,
        out: list[torch.Tensor | None],
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
        frames_by_request_position: dict[int, list[torch.Tensor | None]] = {}
        for request_position in request_positions:
            request = requests[request_position]
            frames_by_request_position[request_position] = [None] * len(request.output_row_indices)
            for output_slot, output_row in enumerate(request.output_row_indices):
                packet_position = int(np.searchsorted(packet_row_indices, output_row, side="right")) - 1
                if packet_position < 0 or int(packet_row_indices[packet_position]) < request.decode_row_indices[0]:
                    continue
                output_slots_by_packet_position.setdefault(packet_position, []).append((request_position, output_slot))

        if not output_slots_by_packet_position:
            return

        # 3. Walk the GOP once, retaining only frames requested by at least one output slot.
        wanted_packet_positions = sorted(output_slots_by_packet_position)
        packets_to_decode = samples[: wanted_packet_positions[-1] + 1]
        decoded_frames_by_packet_position = self._feed_run(
            segment_id,
            packets_to_decode,
            wanted_packet_positions,
        )
        if decoded_frames_by_packet_position is None:
            # Delayed stream with multiple wanted frames: emission order didn't
            # map 1:1 to samples, so decode each wanted frame separately from
            # the run's keyframe.
            decoded_frames_by_packet_position = {}
            for packet_position in wanted_packet_positions:
                decoded_frames_by_packet_position[packet_position] = self._feed_last(
                    segment_id,
                    samples[: packet_position + 1],
                )

        # 4. Scatter captured frames back into each request's ordered output slots.
        for packet_position, tensor in decoded_frames_by_packet_position.items():
            for request_position, output_slot in output_slots_by_packet_position[packet_position]:
                frames_by_request_position[request_position][output_slot] = (
                    tensor if batch.is_windowed else tensor.clone()
                )

        for request_position, frames in frames_by_request_position.items():
            if any(frame is None for frame in frames):
                continue
            resolved = cast("list[torch.Tensor]", frames)
            out[request_position] = torch.stack(resolved) if batch.is_windowed else resolved[0]

    def _feed_run(
        self,
        segment_id: str,
        feed: list[bytes],
        wanted: list[int],
    ) -> dict[int, torch.Tensor] | None:
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

        captured: dict[int, torch.Tensor] = {}
        wanted_set = set(wanted)
        if session.last_frame is not None and session.frames_emitted - 1 in wanted_set:
            # The session's last emitted frame is still at hand (repeated target).
            captured[session.frames_emitted - 1] = self._frame_to_tensor(session.last_frame)

        # The session stays popped while feeding, so a raising packet can't
        # leave a corrupt context behind.
        for sample in feed[len(session.fed_samples) :]:
            for frame in session.context.decode(av.Packet(sample)):
                pos = session.frames_emitted
                session.frames_emitted += 1
                session.last_frame = frame
                if pos in wanted_set:
                    captured[pos] = self._frame_to_tensor(frame)
        session.fed_samples = feed

        if session.frames_emitted == len(feed):
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

        # Delayed frames (B-frames or pipelining): the emission-order capture
        # above is unreliable. Flush — a flushed context cannot be re-fed, so
        # no session is kept — and serve only the window's final frame, which
        # is by construction the last frame the codec emits.
        set_current_span_attributes({
            "rerun.dataloader.video.session_kept": False,
            "rerun.dataloader.video.live_sessions": len(self._sessions),
        })
        target_frame = session.last_frame
        for frame in session.context.decode(None):
            target_frame = frame

        if wanted != [len(feed) - 1]:
            return None
        if target_frame is None:
            raise RuntimeError(
                f"Failed to decode target frame for segment {segment_id}: "
                f"{len(feed)} context samples included a keyframe but the decoder "
                "produced no frame."
            )
        return {len(feed) - 1: self._frame_to_tensor(target_frame)}

    def _feed_last(self, segment_id: str, feed: list[bytes]) -> torch.Tensor:
        """
        Feed *feed* through a (possibly cached) session and return its final frame.

        Unlike a multi-frame `_feed_run`, this always resolves: the final frame is
        by construction the last frame the codec emits, so on a delayed stream the
        flush serves it.
        """
        captured = self._feed_run(segment_id, feed, [len(feed) - 1])
        assert captured is not None  # `_feed_run` only returns `None` for multi-frame wants
        return captured[len(feed) - 1]

    def _decode_selected(self, batch: FieldBatch, request: DecodeRequest) -> torch.Tensor | None:
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
        out: list[torch.Tensor | None] = [None]
        self._decode_run(FieldBatch(column=decode_rows, is_windowed=batch.is_windowed), [adjusted], [0], out=out)
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
        decoder_name = _CODEC_TO_DECODER.get(self.codec, self.codec)
        context = cast("av.VideoCodecContext", av.CodecContext.create(decoder_name, "r"))
        if self.thread_count:
            context.thread_count = self.thread_count
        if decoder_name == "libdav1d":
            # dav1d delays output for pipelining by default; the session fast
            # path needs one frame out per packet in.
            context.options = {"max_frame_delay": "1"}
        return context

    def _frame_to_tensor(self, frame: av.VideoFrame) -> torch.Tensor:
        """Convert a PyAV VideoFrame to a `(3, H, W)` uint8 tensor."""
        arr = frame.to_ndarray(format="rgb24")
        return torch.from_numpy(arr).permute(2, 0, 1)
