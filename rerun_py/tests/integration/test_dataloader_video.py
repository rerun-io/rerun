"""
Integration tests for the keyframe-aware video dataloader.

Exercises `RerunMapDataset` + `VideoFrameDecoder` end-to-end against a small
H.264 stream served via `rr.server.Server`, including validation of the
`is_keyframe` column and aligned layouts with known keyframe intervals.
"""

from __future__ import annotations

import pathlib
import pickle
from concurrent.futures import ThreadPoolExecutor
from typing import TYPE_CHECKING, Literal

import numpy as np
import pytest
import rerun as rr
import torch
from rerun.experimental.dataloader import (
    DataSource,
    Field,
    NoShuffle,
    NumericDecoder,
    RerunIterableDataset,
    RerunMapDataset,
    TimeAlignedSampling,
    VideoFrameDecoder,
    Yuv420Frame,
)
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._utils import (
    QueryPlan,
    _discover_prior_keyframes,
    _fetch_keyframe_ranges,
    _fetch_prior_keyframes,
    _fetch_query,
)

if TYPE_CHECKING:
    from pathlib import Path


VIDEO_ASSET = (
    pathlib.Path(__file__).parents[3] / "tests" / "assets" / "video" / "Big_Buck_Bunny_1080_1s_h264_nobframes.mp4"
)


@pytest.mark.parametrize("query_kind", ["data", "markers"])
def test_large_range_queries_preserve_union_semantics(tmp_path: Path, query_kind: str) -> None:
    """A full fetch block's range predicate must remain safe on a query worker thread."""
    ranges = [(4 * index, 4 * index + 1) for index in range(192)]
    ranges[1] = (1, 5)  # Overlapping predicates must not duplicate returned rows.
    expected = sorted({value for lo, hi in ranges for value in range(lo, hi + 1)})
    values = np.arange(770, dtype=np.int64)
    paths = []
    for segment in ("a", "b"):
        path = tmp_path / f"{segment}.rrd"
        paths.append(path)
        with rr.RecordingStream("rerun_example_range_queries", recording_id=segment) as recording:
            recording.save(path)
            indexes = [
                rr.TimeColumn("frame", sequence=values),
                rr.TimeColumn("time", duration=values.astype("timedelta64[ns]")),
            ]
            recording.send_columns("/state", indexes=indexes, columns=rr.Scalars.columns(scalars=values))
            recording.send_columns(
                "/video", indexes=indexes, columns=rr.VideoStream.columns(is_keyframe=values % 3 == 0)
            )

    with (
        rr.server.Server(host="127.0.0.1", datasets={"ranges": paths}) as server,
        ThreadPoolExecutor(max_workers=1) as executor,
    ):
        dataset = server.client().get_dataset("ranges")
        if query_kind == "data":
            plan = QueryPlan(
                fields={"state": Field("/state:Scalars:scalars", decode=NumericDecoder())},
                fetch_requests={},
                query_indices={},
                query_ranges={segment: list(ranges) for segment in ("a", "b")},
                fill_latest_at=False,
            )
            table = executor.submit(_fetch_query, view=dataset, index="frame", plan=plan).result()
            actual = sorted(
                zip(table.column("rerun_segment_id").to_pylist(), table.column("frame").to_pylist(), strict=True)
            )
            assert actual == [(segment, value) for segment in ("a", "b") for value in expected]
        else:
            marker_path = "/video:VideoStream:is_keyframe"
            result = executor.submit(
                _fetch_keyframe_ranges,
                view=dataset,
                index="time",
                paths=[marker_path],
                ranges={segment: list(ranges) for segment in ("a", "b")},
                sample_index=SampleIndex([], ns_per_sample=100, ns_dtype="timedelta64[ns]"),
            ).result()
            expected_markers = [value for value in expected if value % 3 == 0]
            assert {segment: markers.tolist() for segment, markers in result[marker_path].items()} == dict.fromkeys(
                ("a", "b"), expected_markers
            )


@pytest.mark.parametrize("ns_dtype", ["datetime64[ns]", "timedelta64[ns]"])
def test_live_keyframe_discovery_preserves_off_grid_temporal_markers(tmp_path: Path, ns_dtype: str) -> None:
    rrd_path = tmp_path / "off-grid.rrd"
    values = np.array([7, 3001, 6001], dtype=np.int64).astype(ns_dtype)
    time_column = (
        rr.TimeColumn("time", timestamp=values)
        if ns_dtype == "datetime64[ns]"
        else rr.TimeColumn("time", duration=values)
    )
    with rr.RecordingStream("rerun_example_keyframe_discovery", recording_id="off-grid") as recording:
        recording.save(rrd_path)
        recording.send_columns(
            "/video", indexes=[time_column], columns=rr.VideoStream.columns(is_keyframe=[True, True, True])
        )

    segment = SegmentMetadata("off-grid", 5000, 5000, 1, index_min=7)
    sample_index = SampleIndex([segment], ns_per_sample=100, ns_dtype=ns_dtype)
    anchor = np.datetime64(5000, "ns") if ns_dtype == "datetime64[ns]" else np.timedelta64(5000, "ns")
    fields = {"video": Field("/video:VideoStream:sample", decode=VideoFrameDecoder(), window=(-0.0000002, 0))}
    with rr.server.Server(host="127.0.0.1", datasets={"markers": [rrd_path]}) as server:
        dataset = server.client().get_dataset("markers")
        adaptive = _discover_prior_keyframes(
            view=dataset, index="time", fields=fields, located=[(segment, anchor)], sample_index=sample_index
        )
        full = _fetch_prior_keyframes(
            view=dataset, index="time", fields=fields, located=[(segment, anchor)], sample_index=sample_index
        )

    assert adaptive["video"]["off-grid"].tolist() == [3001]
    assert full["video"]["off-grid"].tolist() == [7, 3001]


def _build_h264_rrd(rrd_path: Path, *, log_is_keyframe: bool, dense_keyframes: bool = False) -> list[int]:
    """
    Build an RRD with one VideoStream sample per demuxed packet on a sequence timeline.

    Returns the list of frame indices that are codec keyframes. When
    `log_is_keyframe` is true, also writes keyframe markers, sparsely by default.
    """
    import av
    from av.bitstream import BitStreamFilterContext

    container = av.open(str(VIDEO_ASSET))
    keyframe_indices: list[int] = []
    samples: list[bytes] = []
    try:
        video_stream = container.streams.video[0]
        bsf = BitStreamFilterContext("h264_mp4toannexb", video_stream)

        def absorb(packet: av.Packet) -> None:
            if packet.pts is None or packet.size == 0:
                return
            if packet.is_keyframe:
                keyframe_indices.append(len(samples))
            samples.append(bytes(packet))

        for packet in container.demux(video_stream):
            for out in bsf.filter(packet):
                absorb(out)
    finally:
        container.close()

    assert keyframe_indices, "test asset must contain at least one keyframe"
    assert keyframe_indices[0] == 0, "test asset's first packet must be a keyframe"

    with rr.RecordingStream("rerun_example_test_dataloader_video", recording_id="dataloader-video") as rec:
        rec.save(rrd_path)
        rec.log("/video", rr.VideoStream(codec=rr.VideoCodec.H264), static=True)
        rec.send_columns(
            "/video",
            indexes=[rr.TimeColumn("frame", sequence=list(range(len(samples))))],
            columns=rr.VideoStream.columns(sample=samples),
        )
        # Companion scalar so tests cover the mixed-decoder query path
        # (`prior_keyframe_path` on a non-video decoder must return None, not raise).
        rec.send_columns(
            "/state",
            indexes=[rr.TimeColumn("frame", sequence=list(range(len(samples))))],
            columns=rr.Scalars.columns(scalars=[float(i) for i in range(len(samples))]),
        )
        if log_is_keyframe:
            markers = list(range(len(samples))) if dense_keyframes else keyframe_indices
            keyframe_set = set(keyframe_indices)
            rec.send_columns(
                "/video",
                indexes=[rr.TimeColumn("frame", sequence=markers)],
                columns=rr.VideoStream.columns(is_keyframe=[index in keyframe_set for index in markers]),
            )

    return keyframe_indices


@pytest.fixture
def rrd_with_keyframes(tmp_path: Path) -> tuple[Path, list[int]]:
    rrd_dir = tmp_path / "with_keyframes"
    rrd_dir.mkdir()
    keyframes = _build_h264_rrd(rrd_dir / "recording.rrd", log_is_keyframe=True)
    return rrd_dir, keyframes


@pytest.fixture
def rrd_without_keyframes(tmp_path: Path) -> tuple[Path, list[int]]:
    rrd_dir = tmp_path / "without_keyframes"
    rrd_dir.mkdir()
    keyframes = _build_h264_rrd(rrd_dir / "recording.rrd", log_is_keyframe=False)
    return rrd_dir, keyframes


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
def test_keyframe_path_decodes_mid_gop_target(rrd_with_keyframes: tuple[Path, list[int]]) -> None:
    """The `is_keyframe` column expands a mid-GOP request back to its prior keyframe."""
    rrd_dir, keyframes = rrd_with_keyframes
    target = keyframes[0] + 5
    assert target not in keyframes, "target must sit strictly between keyframes"

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        ds = server.client().get_dataset("video")
        source = DataSource(ds)
        dataset = RerunMapDataset(
            source,
            "frame",
            {
                "image": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264"),
                ),
                "state": Field("/state:Scalars:scalars", decode=NumericDecoder()),
            },
        )
        sample = dataset[target]

    image = sample["image"]
    assert isinstance(image, torch.Tensor)
    assert image.ndim == 3
    assert image.shape[0] == 3  # (C, H, W)
    state = sample["state"]
    assert isinstance(state, torch.Tensor)
    assert float(state[0]) == float(target)


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
@pytest.mark.parametrize("window_storage", ["copy", "view"])
def test_windowed_video_returns_frame_stack(
    rrd_with_keyframes: tuple[Path, list[int]],
    window_storage: Literal["copy", "view"],
) -> None:
    rrd_dir, keyframes = rrd_with_keyframes
    target = keyframes[0] + 5
    window = (-3, -2, -1, 0)

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        source = DataSource(server.client().get_dataset("video"))
        dataset = RerunMapDataset(
            source,
            "frame",
            {
                "clip": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(
                        codec="h264",
                        window_storage=window_storage,
                    ),
                    window=window,
                ),
                "single": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264"),
                ),
            },
        )
        sample = dataset[target]

    clip = sample["clip"]
    frame = sample["single"]
    assert isinstance(clip, torch.Tensor)
    assert isinstance(frame, torch.Tensor)
    assert tuple(clip.shape[:2]) == (len(window), 3)
    assert clip.shape[2:] == frame.shape[1:]
    assert torch.equal(clip[-1], frame)


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
def test_windowed_video_can_remain_yuv420(rrd_with_keyframes: tuple[Path, list[int]]) -> None:
    """The YUV output crosses the complete map-dataset pipeline without implicit RGB conversion."""
    rrd_dir, keyframes = rrd_with_keyframes
    target = keyframes[0] + 5
    window = (-3, -2, -1, 0)

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        source = DataSource(server.client().get_dataset("video"))
        dataset = RerunMapDataset(
            source,
            "frame",
            {
                "clip": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264", window_storage="view", output_format="yuv420p"),
                    window=window,
                ),
            },
        )
        clip = dataset[target]["clip"]

    assert isinstance(clip, Yuv420Frame)
    assert clip.y.shape[:2] == (len(window), 1)
    assert clip.uv.shape[:2] == (len(window), 2)
    assert clip.y.shape[-2] == clip.uv.shape[-2] * 2
    assert clip.y.shape[-1] == clip.uv.shape[-1] * 2


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
@pytest.mark.parametrize("dataset_type", [RerunMapDataset, RerunIterableDataset])
@pytest.mark.parametrize("timeline_sampling", [None, TimeAlignedSampling(keyframe_interval=30)])
def test_missing_is_keyframe_column_raises(
    rrd_without_keyframes: tuple[Path, list[int]],
    dataset_type: type[RerunMapDataset] | type[RerunIterableDataset],
    timeline_sampling: TimeAlignedSampling | None,
) -> None:
    """A video field without its sibling `is_keyframe` column is rejected before fetching samples."""
    rrd_dir, keyframes = rrd_without_keyframes
    target = keyframes[0] + 5

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        ds = server.client().get_dataset("video")
        source = DataSource(ds)
        dataset = dataset_type(
            source,
            "frame",
            {
                "image": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264"),
                ),
                "state": Field("/state:Scalars:scalars", decode=NumericDecoder()),
            },
            timeline_sampling=timeline_sampling,
        )
        with pytest.raises(ValueError, match="require an is_keyframe column"):
            if isinstance(dataset, RerunMapDataset):
                dataset[target]
            else:
                next(iter(dataset))


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
def test_aligned_layout_decodes_identical_samples_with_keyframe_markers(
    rrd_with_keyframes: tuple[Path, list[int]],
) -> None:
    rrd_dir, keyframes = rrd_with_keyframes
    assert keyframes == [0]

    fields = {
        "image": Field("/video:VideoStream:sample", decode=VideoFrameDecoder(codec="h264"), window=(0, -2)),
        "actions": Field("/state:Scalars:scalars", decode=NumericDecoder(), window=(1, 2)),
    }
    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        source = DataSource(server.client().get_dataset("video"))
        reference = RerunMapDataset(source, "frame", fields)
        aligned = RerunMapDataset(
            source,
            "frame",
            fields,
            timeline_sampling=TimeAlignedSampling(keyframe_interval=30),
        )
        # The asset has 30 frames; the windows trim two anchors from each end.
        assert len(reference) == len(aligned) == 26
        positions = [25, 0, 3]
        expected = reference.__getitems__(positions)
        actual = pickle.loads(pickle.dumps(aligned)).__getitems__(positions)

    for anchor, expected_sample, actual_sample in zip([27, 2, 5], expected, actual, strict=True):
        for field in fields:
            expected_value = expected_sample[field]
            actual_value = actual_sample[field]
            assert isinstance(expected_value, torch.Tensor)
            assert isinstance(actual_value, torch.Tensor)
            assert torch.equal(actual_value, expected_value)
        actions = actual_sample["actions"]
        assert isinstance(actions, torch.Tensor)
        assert actions.tolist() == [[float(anchor + 1)], [float(anchor + 2)]]


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
@pytest.mark.parametrize("dataset_type", [RerunMapDataset, RerunIterableDataset])
@pytest.mark.parametrize("dense_keyframes", [False, True])
def test_aligned_sampling_marks_unmarked_decode_start_incomplete(
    tmp_path: Path, dataset_type: type[RerunMapDataset] | type[RerunIterableDataset], dense_keyframes: bool
) -> None:
    rrd_path = tmp_path / "recording.rrd"
    keyframes = _build_h264_rrd(rrd_path, log_is_keyframe=True, dense_keyframes=dense_keyframes)
    assert keyframes == [0]

    with rr.server.Server(datasets={"video": [rrd_path]}) as server:
        source = DataSource(server.client().get_dataset("video"))
        fields = {"image": Field("/video:VideoStream:sample", decode=VideoFrameDecoder(codec="h264"))}
        sampling = TimeAlignedSampling(keyframe_interval=10)
        if dataset_type is RerunMapDataset:
            dataset = RerunMapDataset(source, "frame", fields, timeline_sampling=sampling)
            reference = dataset[9]["image"]
            assert isinstance(reference, torch.Tensor)
            for positions in ([10], [9, 10], [10, 9, 10]):
                samples = dataset.__getitems__(positions)
                for position, sample in zip(positions, samples, strict=True):
                    image = sample["image"]
                    if position == 10:
                        assert image is None
                    else:
                        assert isinstance(image, torch.Tensor)
                        assert torch.equal(image, reference)
        else:
            with pytest.warns(RuntimeWarning, match="field 'image' has no value"):
                samples = list(
                    RerunIterableDataset(
                        source, "frame", fields, timeline_sampling=sampling, shuffle_strategy=NoShuffle()
                    )
                )
            assert len(samples) == 10
            assert all(isinstance(sample["image"], torch.Tensor) for sample in samples)
            strict = iter(
                RerunIterableDataset(
                    source,
                    "frame",
                    fields,
                    timeline_sampling=sampling,
                    shuffle_strategy=NoShuffle(),
                    max_consecutive_skipped_samples=0,
                )
            )
            for _ in range(10):
                assert isinstance(next(strict)["image"], torch.Tensor)
            with pytest.warns(RuntimeWarning, match="field 'image' has no value"):
                with pytest.raises(RuntimeError, match="max_consecutive_skipped_samples=0"):
                    next(strict)


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
def test_aligned_sampling_does_not_fill_missing_numeric_values(tmp_path: Path) -> None:
    rrd_path = tmp_path / "sparse.rrd"
    with rr.RecordingStream("rerun_example_test_aligned_sampling", recording_id="sparse") as rec:
        rec.save(rrd_path)
        rec.send_columns(
            "/state",
            indexes=[rr.TimeColumn("frame", sequence=[0, 2])],
            columns=rr.Scalars.columns(scalars=[10.0, 30.0]),
        )

    fields = {"state": Field("/state:Scalars:scalars", decode=NumericDecoder())}
    with rr.server.Server(datasets={"sparse": [rrd_path]}) as server:
        source = DataSource(server.client().get_dataset("sparse"))
        default = RerunMapDataset(source, "frame", fields)
        aligned = RerunMapDataset(source, "frame", fields, timeline_sampling=TimeAlignedSampling(keyframe_interval=10))
        positions = [2, 1, 0, 2]
        default_samples = default.__getitems__(positions)
        aligned_samples = aligned.__getitems__(positions)

    for samples, expected in [(default_samples, [30.0, 10.0, 10.0, 30.0]), (aligned_samples, [30.0, None, 10.0, 30.0])]:
        values = []
        for sample in samples:
            state = sample["state"]
            assert state is None or isinstance(state, torch.Tensor)
            values.append(None if state is None else state.item())
        assert values == expected
