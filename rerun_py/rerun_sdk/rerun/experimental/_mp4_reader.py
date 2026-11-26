from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Literal, overload

import numpy as np
import pyarrow as pa

from rerun_bindings import Mp4ReaderInternal, Mp4TranscodeOptionsInternal

from ..chunk import LazyChunkStream
from ..components import VideoCodec

if TYPE_CHECKING:
    import numpy.typing as npt

    from rerun_bindings.types import TemporalTimelineType


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


@dataclass(frozen=True, kw_only=True)
class Mp4Info:
    """
    Information about an MP4 file as stored, taken from its container metadata.

    It describes the source file, and is unaffected by the reader's `mode`, `transcode`, and timeline
    settings. A stream that gets transcoded emits different samples, and its keyframes may differ from the
    ones listed here.
    """

    codec: VideoCodec
    """The codec of the video track."""

    codec_string: str
    """The codec in the form used by the WebCodecs codec registry, e.g. `avc1.64001f`."""

    width: int
    """The width of the video in pixels."""

    height: int
    """The height of the video in pixels."""

    bit_depth: int | None
    """The bit depth per color component, or `None` if the metadata doesn't state it."""

    num_frames: int
    """The number of frames, which is the number of rows of `samples`."""

    duration_ns: int
    """
    The duration of the video track as declared by the container, in nanoseconds.

    It includes the time the last frame is presented, so it is longer than the span between the first
    and the last presentation time. It can be longer still when the file has B-frames.
    """

    has_b_frames: bool
    """
    Whether any frame is decoded at a different time than it is presented.

    `mode="stream"` always transcodes such a file with FFmpeg.
    """

    gop_size_min: int
    """
    The number of frames in the smallest GOP, the last one included.

    Frames before the first keyframe belong to no GOP. `0` if the video has no keyframe.
    """

    gop_size_max: int
    """
    The number of frames in the largest GOP, the last one included.

    Frames before the first keyframe belong to no GOP. `0` if the video has no keyframe.
    """

    samples: pa.RecordBatch = field(repr=False, hash=False)
    """
    One row per frame, in decode order, which is the order of the frames in the file.

    | Column        | Type           | Meaning                                  |
    | ------------- | -------------- | ---------------------------------------- |
    | `frame_nr`    | `int64`        | Index of the frame in presentation order |
    | `pts`         | `duration[ns]` | Presentation time                        |
    | `dts`         | `duration[ns]` | Decode time, which can be negative       |
    | `duration`    | `duration[ns]` | Frame duration                           |
    | `is_keyframe` | `bool`         | Whether the frame starts a GOP           |
    | `byte_offset` | `uint64`       | Offset of the frame's bytes in the file  |
    | `byte_size`   | `uint64`       | Length of the frame's bytes              |

    `pts` holds the values that `mode="stream"` puts on the video timeline, as long as the stream is
    not transcoded.
    """

    @property
    def keyframe_timestamps_ns(self) -> npt.NDArray[np.int64]:
        """The presentation times of the keyframes in nanoseconds, ascending."""
        return np.sort(self._keyframes().column("pts").cast(pa.int64()).to_numpy())

    @property
    def keyframe_frame_nrs(self) -> npt.NDArray[np.int64]:
        """The `frame_nr` of each keyframe, ascending."""
        return np.sort(self._keyframes().column("frame_nr").to_numpy())

    def _keyframes(self) -> pa.RecordBatch:
        return self.samples.filter(self.samples.column("is_keyframe"))


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
        timeline_type: TemporalTimelineType = "duration_ns",
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
        timeline_type: TemporalTimelineType = "duration_ns",
        entity_path: str | None = None,
    ) -> None: ...

    def __init__(
        self,
        path: str | Path,
        *,
        mode: Literal["asset", "stream"] = "stream",
        chunk_by_gop: bool = True,
        timeline_name: str = "video",
        timeline_type: TemporalTimelineType = "duration_ns",
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

            - `"stream"` (default): emit a static `VideoStream(codec=…)` chunk,
              then per-GOP (or per-sample) `VideoSample` chunks, then one
              `VideoStream:is_keyframe` chunk with a `True` row per keyframe.
              The mp4 must use a codec representable as
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
            `"duration"` and `"timestamp"` are accepted as aliases of `"duration_ns"` and `"timestamp_ns"`.

            The emitted values are the mp4 PTS (nanoseconds since the start of
            the video) only the declared Arrow type changes:

            - `"duration_ns"` (default): the values are typed as a duration, the
              natural mp4 PTS interpretation.
            - `"timestamp_ns"`: the same PTS values, typed as nanoseconds since the
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

    def info(self) -> Mp4Info:
        """
        Return information about the MP4 file as stored.

        Only the container metadata is read, not the video data, so this is much cheaper than
        streaming the file. VP8 and VP9 are the exception: their keyframe flags are recovered from
        the video data, which reads every frame.

        The reader's `mode`, `transcode`, and timeline settings do not affect the result.
        The file is read again on each call.

        Raises
        ------
        RuntimeError
            If the file is not a valid MP4, or its video codec is not supported.

        """
        internal = self._internal.info()
        return Mp4Info(
            codec=VideoCodec(internal.codec),
            codec_string=internal.codec_string,
            width=internal.width,
            height=internal.height,
            bit_depth=internal.bit_depth,
            num_frames=internal.num_frames,
            duration_ns=internal.duration_ns,
            has_b_frames=internal.has_b_frames,
            gop_size_min=internal.gop_size_min,
            gop_size_max=internal.gop_size_max,
            samples=internal.samples,
        )

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
