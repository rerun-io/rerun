"""Decoder for compressed video columns, with context-aware random access."""

from __future__ import annotations

from collections import OrderedDict
from typing import TYPE_CHECKING, Any, cast

import av
import numpy as np
import torch

from rerun._tracing import set_current_span_attributes, with_tracing

from ....components import VideoCodec
from ...video import detect_gop_start, is_annex_b, length_prefixed_to_annex_b
from .._sample_index import IndexValue, _ns_to_datetime64, _ns_to_timedelta64
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
    Group request positions into runs whose decode windows chain into one contiguous row window.

    A run never crosses a segment boundary: rows restart per segment, so two
    segments' windows may not be chained, and each segment's codec session is
    keyed separately.
    """
    runs: list[list[int]] = []
    for i, request in enumerate(requests):
        previous = requests[runs[-1][-1]] if runs else None
        if (
            previous is not None
            and request.segment_id == previous.segment_id
            and request.rows.start <= previous.rows.stop
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
    Extract the encoded samples of `rows`, plus each kept sample's row.

    Skips empty rows, converts length-prefixed H.264 to Annex B, and drops
    consecutive duplicate samples: `fill_latest_at` repeats the previous
    frame's bytes for grid slots with no source frame, and re-feeding a
    duplicate packet corrupts the decoder's reference state. A dropped
    duplicate's row is dropped with it, so a request landing on the duplicate
    resolves to the kept sample holding the same bytes.
    """
    # TODO(RR-4751): we should measure whether we can optimize this by doing precise queries when `VideoStream::is_keyframe` is present.
    samples: list[bytes] = []
    kept_rows: list[int] = []
    for row in rows:
        sample_bytes = bytes(_flatten_blob(column, row))
        if not sample_bytes:
            continue
        if video_codec is VideoCodec.H264 and not is_annex_b(sample_bytes):
            sample_bytes = length_prefixed_to_annex_b(sample_bytes)
        if samples and sample_bytes == samples[-1]:
            continue
        samples.append(sample_bytes)
        kept_rows.append(row)
    return samples, kept_rows


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

    A batch's requests are grouped by GOP: each GOP's packets are extracted
    and fed through the codec once, and every requested frame is captured as
    it is emitted. A batch (or a later batch) whose window extends an earlier
    one reuses the open codec context and decodes only the new packets.

    Returns `None` when a request's resolved window contains no decodable
    keyframe: the target precedes the entity's first frame in a multi-modal
    segment, the fallback `keyframe_interval` under-estimates the true GOP
    length, or the anchored row was user-logged `is_keyframe=true` on a sample
    that isn't actually a codec keyframe (run optimize with
    `fix_keyframe=True` to re-derive markers from the encoded samples).
    Consumers must filter these out in their collate function before stacking.
    """

    def __init__(
        self,
        *,
        keyframe_interval: int = 30,
        fps_estimate: float = 30.0,
        codec: str = "h264",
        max_decoder_sessions: int = 8,
        thread_count: int = 1,
    ) -> None:
        """
        Construct a decoder for a compressed video column.

        Parameters
        ----------
        keyframe_interval:
            Fallback GOP length (in frames) used to estimate how far back the
            prior keyframe sits when the stream has no explicit markers.
        fps_estimate:
            Fallback frame rate used to turn `keyframe_interval` into a time window.
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
        self._keyframe_interval = keyframe_interval
        self._fps_estimate = fps_estimate
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

    def context_range(
        self,
        index_value: IndexValue,
    ) -> tuple[IndexValue, IndexValue] | None:
        """Need frames from estimated keyframe position to target."""
        keyframe_duration_ns = int(self._keyframe_interval / self._fps_estimate * 1e9)
        if isinstance(index_value, np.datetime64):
            iv = int(np.int64(index_value))
            return (_ns_to_datetime64(iv - keyframe_duration_ns), index_value)
        if isinstance(index_value, np.timedelta64):
            iv = int(np.int64(index_value))
            return (_ns_to_timedelta64(iv - keyframe_duration_ns), index_value)
        iv = int(index_value)
        return (max(0, iv - self._keyframe_interval), iv)

    @with_tracing("VideoFrameDecoder.decode")
    def decode(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> list[torch.Tensor | None]:
        """Decode every request's frame, feeding each GOP through the codec once."""
        out: list[torch.Tensor | None] = [None] * len(requests)
        if batch.select is not None:
            # A selector may change row counts, which breaks the row <-> sample
            # mapping the GOP batching relies on; decode per request instead.
            for i, request in enumerate(requests):
                out[i] = self._decode_selected(batch, request)
            return out

        runs = _decode_runs(requests)
        set_current_span_attributes({
            "rerun.dataloader.video.num_requests": len(requests),
            "rerun.dataloader.video.num_segments": len({request.segment_id for request in requests}),
            "rerun.dataloader.video.gop_runs": len(runs),
        })
        for run in runs:
            self._decode_run(batch, requests, run, out=out)
        return out

    def _decode_run(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
        run: list[int],
        *,
        out: list[torch.Tensor | None],
    ) -> None:
        """Decode one run of requests whose windows form a single contiguous GOP walk."""
        segment_id = requests[run[0]].segment_id
        # A run's windows chain into one contiguous row window, so its row
        # span is just the union of its requests' spans.
        start = min(requests[i].rows.start for i in run)
        stop = max(requests[i].rows.stop for i in run)
        samples, kept_rows = _extract_video_samples(batch.column, range(start, stop), video_codec=self._video_codec)

        drop = self._num_leading_non_keyframes(samples)
        samples = samples[drop:]
        kept_rows = kept_rows[drop:]
        if not samples:
            # No bootstrap context anywhere in the run's window: every request in the run cold-starts.
            return

        # Per-sample keyframe positions, so each request can be checked against
        # its *own* window: a heuristic run's window may span keyframes an
        # individual request's window doesn't contain, and such a request must
        # still return `None` exactly as a per-sample decode would. Anchored
        # requests start their window at the prior keyframe, so the trimmed
        # window already encodes decodability (`pos < 0` below) and the
        # per-sample scan is skipped.
        has_detector = self._is_keyframe(samples[0]) is not None
        check_windows = has_detector and any(not requests[i].starts_at_keyframe for i in run)
        prior_keyframe_pos: list[int] = [0] * len(samples)
        if check_windows:
            last_kf = -1
            for pos, sample in enumerate(samples):
                if self._is_keyframe(sample):
                    last_kf = pos
                prior_keyframe_pos[pos] = last_kf

        # Map each request to the kept sample holding its frame: the last kept
        # row inside its window (the target, unless the field is explicitly
        # windowed).
        kept = np.asarray(kept_rows, dtype=np.int64)
        slots_by_pos: dict[int, list[int]] = {}
        for i in run:
            request = requests[i]
            pos = int(np.searchsorted(kept, request.rows.stop, side="left")) - 1
            if pos < 0:
                continue
            if check_windows:
                kf_pos = prior_keyframe_pos[pos]
                if kf_pos < 0 or int(kept[kf_pos]) < request.rows.start:
                    # No keyframe inside this request's own window: cold start.
                    continue
            slots_by_pos.setdefault(pos, []).append(i)

        if not slots_by_pos:
            return

        wanted = sorted(slots_by_pos)
        feed = samples[: wanted[-1] + 1]
        captured = self._feed_run(segment_id, feed, wanted)
        if captured is None:
            # Delayed stream with multiple wanted frames: emission order didn't
            # map 1:1 to samples, so decode each wanted frame separately from
            # its own prior keyframe (the pre-batch per-sample behavior).
            captured = {}
            for pos in wanted:
                first = max(0, prior_keyframe_pos[pos]) if check_windows else 0
                captured[pos] = self._feed_last(segment_id, samples[first : pos + 1])

        for pos, tensor in captured.items():
            slots = slots_by_pos[pos]
            out[slots[0]] = tensor
            for slot in slots[1:]:
                # Grid slots that snapped to the same source frame must not share storage.
                out[slot] = tensor.clone()

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
        """Decode one request whose window went through a selector."""
        raw = batch.raw(request)
        samples, _ = _extract_video_samples(raw, range(len(raw)), video_codec=self._video_codec)

        samples = samples[self._num_leading_non_keyframes(samples) :]
        if not samples:
            # No bootstrap context anywhere in the window. See class docstring.
            return None

        return self._feed_last(request.segment_id, samples)

    def _num_leading_non_keyframes(self, samples: list[bytes]) -> int:
        """
        Number of leading samples to drop so the decoder sees a bootstrap packet first.

        libdav1d rejects a non-keyframe outright; H.264/HEVC need SPS/PPS, plus
        VPS for HEVC, before any non-IDR/IRAP slice. For codecs without a
        detector, `_is_keyframe` returns `None` and nothing is dropped (we
        trust the decoder). `len(samples)` when no sample can bootstrap.
        """
        drop = 0
        while drop < len(samples):
            is_keyframe = self._is_keyframe(samples[drop])
            if is_keyframe is None or is_keyframe:
                break
            drop += 1
        return drop

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
