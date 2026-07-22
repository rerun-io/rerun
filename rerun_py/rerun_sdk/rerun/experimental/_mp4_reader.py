from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Literal, overload

from rerun_bindings import Mp4ReaderInternal, Mp4TranscodeOptionsInternal

from ..components import VideoCodec
from ._lazy_chunk_stream import LazyChunkStream


@dataclass(frozen=True, kw_only=True)
class Mp4TranscodeOptions:
    """How to transcode an mp4."""

    output_codec: VideoCodec | None = None
    """
    Re-encode to this [`VideoCodec`][rerun.components.VideoCodec] instead of keeping
    the source codec; the emitted `VideoStream` codec follows it. `None` (default)
    keeps the source codec.
    """

    gop_size: int | None = None
    """
    Force a keyframe every `gop_size` frames in the transcoded output. Requesting it
    triggers a re-encode. `None` (default) keeps the encoder's default GOP.
    """

    try_gpu: bool = False
    """
    Try to use a hardware (GPU) encoder if the local FFmpeg provides one for the
    output codec, otherwise fall back to software (best-effort). GPU encoding is
    drawn from the NVENC and VideoToolbox families only, so it realistically applies
    to H264/H265 and, on newer NVIDIA hardware, AV1. VP8/VP9 always fall back to
    software — their only GPU encoders are Intel QSV/VAAPI, which are not yet used.
    Has no effect unless a transcode is already happening.
    """

    ffmpeg_override: str | Path | None = None
    """
    Override the `ffmpeg` executable used to transcode. When `None` (default),
    `ffmpeg` is looked up on `PATH`. Ignored when no transcode happens.
    """

    def __post_init__(self) -> None:
        if self.output_codec is not None and not isinstance(self.output_codec, VideoCodec):
            raise TypeError(
                f"output_codec must be a rerun.components.VideoCodec, got {type(self.output_codec).__name__}"
            )

    def _to_internal(self) -> Mp4TranscodeOptionsInternal:
        """Lower the validated options onto the Rust binding (codec → its fourcc value)."""
        return Mp4TranscodeOptionsInternal(
            gop_size=self.gop_size,
            output_codec=None if self.output_codec is None else self.output_codec.value,
            try_gpu=self.try_gpu,
            ffmpeg_override=None if self.ffmpeg_override is None else Path(self.ffmpeg_override).absolute(),
        )


class Mp4Reader:
    """Read chunks from an MP4 file."""

    _internal: Mp4ReaderInternal

    @overload
    def __init__(
        self,
        path: str | Path,
        *,
        mode: Literal["stream"] = "stream",
        chunk_by_gop: bool = True,
        timeline_name: str = "video",
        timeline_type: Literal["duration", "timestamp"] = "duration",
        transcode: Mp4TranscodeOptions | None = None,
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
        transcode: Mp4TranscodeOptions | None = None,
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
              [`VideoCodec`][rerun.components.VideoCodec].
              A source containing B-frames — or any source for which a
              transform is requested via `transcode` — is transcoded with FFmpeg
              into an equivalent B-frame-free stream before emission, which
              requires an `ffmpeg` executable.
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
            How to interpret the timeline values.

            The emitted values are the mp4 PTS (nanoseconds since the start of
            the video) only the declared Arrow type changes:

            - `"duration"` (default): the values are typed as a duration, the
              natural mp4 PTS interpretation.
            - `"timestamp"`: the same PTS values, typed as nanoseconds since the
              Unix epoch. The reader does not shift them, so until you retag them
              — via a downstream `.map(...)` on the chunk stream with
              caller-supplied wall-clock times (e.g. from a trajectory file) —
              they render as timestamps near 1970.
        transcode:
            Only meaningful when `mode="stream"`. An
            [`Mp4TranscodeOptions`][rerun.experimental.Mp4TranscodeOptions]
            describing an optional re-encode.
        entity_path:
            Entity path under which chunks are emitted. When `None` (default),
            the entity path is derived from the absolute file path (e.g.
            `foo/video.mp4` run from `/data` becomes `/data/foo/video.mp4`). The
            path is resolved to absolute up front, so the result is independent
            of any later change to the working directory.

        """
        if mode == "asset" and transcode is not None:
            raise ValueError('`transcode` is only valid with `mode="stream"`')

        self._internal = Mp4ReaderInternal(
            Path(path).absolute(),
            mode=mode,
            chunk_by_gop=chunk_by_gop,
            timeline_name=timeline_name,
            timeline_type=timeline_type,
            transcode=None if transcode is None else transcode._to_internal(),
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
