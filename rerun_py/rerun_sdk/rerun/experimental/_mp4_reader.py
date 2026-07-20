from __future__ import annotations

from pathlib import Path
from typing import Literal, overload

from rerun_bindings import Mp4ReaderInternal

from ._lazy_chunk_stream import LazyChunkStream


class Mp4Reader:
    """Read chunks from an MP4 file."""

    _internal: Mp4ReaderInternal

    # `chunk_by_gop` and `ffmpeg_override` only apply to `mode="stream"`. The
    # overloads encode that so the type checker rejects, e.g.,
    # `Mp4Reader(path, mode="asset", chunk_by_gop=False)` — which the constructor
    # would otherwise reject only at runtime. `timeline_name` and `timeline_type`
    # apply to both modes, so they appear in both overloads.
    @overload
    def __init__(
        self,
        path: str | Path,
        *,
        mode: Literal["stream"] = "stream",
        chunk_by_gop: bool = True,
        timeline_name: str = "video",
        timeline_type: Literal["duration", "timestamp"] = "duration",
        ffmpeg_override: str | Path | None = None,
        entity_path: str | None = None,
    ) -> None: ...

    @overload
    def __init__(
        self,
        path: str | Path,
        *,
        mode: Literal["asset"],
        timeline_name: str = "video",
        timeline_type: Literal["duration", "timestamp"] = "duration",
        entity_path: str | None = None,
    ) -> None: ...

    def __init__(
        self,
        path: str | Path,
        *,
        mode: Literal["asset", "stream"] = "stream",
        chunk_by_gop: bool = True,
        timeline_name: str = "video",
        timeline_type: Literal["duration", "timestamp"] = "duration",
        ffmpeg_override: str | Path | None = None,
        entity_path: str | None = None,
    ) -> None:
        """
        Construct a new MP4 reader.

        Parameters
        ----------
        path:
            Path to the `.mp4` file to read.
        mode:
            How to convert the mp4 into chunks.

            - `"stream"` (default): emit a static `VideoStream(codec=…)` chunk
              followed by per-GOP (or per-sample) `VideoSample` chunks. The mp4
              must use a codec representable as
              [`VideoCodec`][rerun.components.VideoCodec] (H264, H265, AV1, VP8,
              VP9). A source containing B-frames is transcoded with FFmpeg into
              an equivalent B-frame-free stream before emission, which requires
              an `ffmpeg` executable (see `ffmpeg_override`).
            - `"asset"`: emit an `AssetVideo` blob chunk plus a
              `VideoFrameReference` index chunk, matching the behavior of
              `rerun video.mp4`.
        chunk_by_gop:
            Only meaningful when `mode="stream"`. When `True` (default), each
            emitted Rerun chunk contains a keyframe plus all dependent samples
            up to (but not including) the next keyframe. When `False`, each
            sample becomes its own one-row Rerun chunk.

            Passing `chunk_by_gop=False` together with `mode="asset"` raises
            `ValueError`.
        timeline_name:
            Name of the timeline used for stream-mode samples and for the
            `VideoFrameReference` index chunk in asset mode. Defaults to
            `"video"`.
        timeline_type:
            How to interpret the timeline values. Applies to both modes (the
            stream-mode sample timeline and the asset-mode `VideoFrameReference`
            index timeline).

            The emitted values are the mp4 PTS (nanoseconds since the start of
            the video) in *both* cases — only the declared Arrow type changes:

            - `"duration"` (default): the values are typed as a duration, the
              natural mp4 PTS interpretation.
            - `"timestamp"`: the same PTS values, typed as nanoseconds since the
              Unix epoch. The reader does not shift them, so until you retag them
              — via a downstream `.map(...)` on the chunk stream with
              caller-supplied wall-clock times (e.g. from a trajectory file) —
              they render as timestamps near 1970.
        ffmpeg_override:
            Only meaningful when `mode="stream"`. Overrides the `ffmpeg`
            executable used to transcode a B-frame source. When `None`
            (default), `ffmpeg` is looked up on `PATH`. If the source has no
            B-frames, no transcode happens and this is ignored.
        entity_path:
            Entity path under which chunks are emitted. When `None` (default),
            the entity path is derived from the absolute file path (e.g.
            `foo/video.mp4` run from `/data` becomes `/data/foo/video.mp4`). The
            path is resolved to absolute up front, so the result is independent
            of any later change to the working directory.

        """
        self._internal = Mp4ReaderInternal(
            Path(path).absolute(),
            mode=mode,
            chunk_by_gop=chunk_by_gop,
            timeline_name=timeline_name,
            timeline_type=timeline_type,
            ffmpeg_override=None if ffmpeg_override is None else Path(ffmpeg_override).absolute(),
            entity_path=entity_path,
        )

    def stream(self) -> LazyChunkStream:
        """Return a lazy stream over all chunks in the MP4 file."""
        return LazyChunkStream(self._internal.stream())

    @property
    def path(self) -> Path:
        """The file path of the MP4 file."""
        return self._internal.path

    @property
    def entity_path(self) -> str:
        """The entity path under which chunks are emitted."""
        return self._internal.entity_path

    def __repr__(self) -> str:
        return f"Mp4Reader({self._internal.path})"
