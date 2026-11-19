from __future__ import annotations

import json
from typing import TYPE_CHECKING, cast

import pyarrow as pa
import pytest
import rerun as rr
import torch
from datafusion import SessionContext
from rerun.experimental.dataloader import (
    DataSource,
    Field,
    FixedRateSampling,
    NoShuffle,
    NumericDecoder,
    RerunIterableDataset,
    RerunMapDataset,
    TimeAlignedSampling,
)
from rerun.experimental.dataloader._sample_index import SampleIndex, _index_value_to_int
from rerun.experimental.spans import Curation

if TYPE_CHECKING:
    from collections.abc import Generator
    from pathlib import Path

    from rerun.catalog import DatasetEntry

pytestmark = pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")

_DatasetType = type[RerunMapDataset] | type[RerunIterableDataset]


@pytest.fixture
def source_dataset(tmp_path: Path) -> Generator[DatasetEntry, None, None]:
    paths = []
    for segment, base in (("a", 0), ("b", 10)):
        path = tmp_path / f"{segment}.rrd"
        with rr.RecordingStream("rerun_example_dataloader_source", recording_id=segment) as recording:
            recording.save(path)
            for entity, count in (("value", 4), ("other", 6)):
                recording.send_columns(
                    entity,
                    indexes=[
                        rr.TimeColumn("frame", sequence=range(count)),
                        rr.TimeColumn("frame2", sequence=range(count)),
                        rr.TimeColumn("time", duration=[float(index) for index in range(count)]),
                        rr.TimeColumn("stamp", timestamp=[0.25 + index for index in range(count)]),
                    ],
                    columns=rr.Scalars.columns(scalars=[base + index for index in range(count)]),
                )
        paths.append(path)
    with rr.server.Server(host="127.0.0.1", datasets={"sources": paths}) as server:
        yield server.client().get_dataset("sources")


def _curation(
    dataset: DatasetEntry,
    spans: list[tuple[str, int, int]],
    *,
    index: str = "frame",
    datatype: pa.DataType | None = None,
) -> Curation:
    datatype = pa.int64() if datatype is None else datatype
    descriptor: dict[str, str | None] = {"kind": "sequence"}
    if pa.types.is_duration(datatype):
        descriptor = {"kind": "duration"}
    elif pa.types.is_timestamp(datatype):
        descriptor = {"kind": "timestamp", "timezone": datatype.tz}
    return Curation.from_arrow(
        dataset,
        pa.table(
            {
                "segment_id": pa.array([segment for segment, _, _ in spans], pa.string()),
                "start": pa.array([start for _, start, _ in spans], datatype),
                "end": pa.array([end for _, _, end in spans], datatype),
            },
            metadata={
                "manifest_format_version": "3",
                "dataset_id": str(dataset.id),
                "index_name": index,
                "index_type": json.dumps(descriptor),
            },
        ),
    )


def _loader(
    dataset_type: _DatasetType, source: DataSource, *, entity: str = "value"
) -> RerunMapDataset | RerunIterableDataset:
    fields = {"value": Field(f"/{entity}:Scalars:scalars", decode=NumericDecoder())}
    if dataset_type is RerunIterableDataset:
        return RerunIterableDataset(source, "frame", fields, shuffle_strategy=NoShuffle())
    return RerunMapDataset(source, "frame", fields)


def _values(dataset: RerunMapDataset | RerunIterableDataset) -> list[float]:
    samples = dataset.__getitems__(list(range(len(dataset)))) if isinstance(dataset, RerunMapDataset) else list(dataset)
    return [float(cast("torch.Tensor", sample["value"]).item()) for sample in samples]


@pytest.mark.parametrize("dataset_type", [RerunMapDataset, RerunIterableDataset])
def test_dataset_and_selection_load_without_annotations(
    source_dataset: DatasetEntry, dataset_type: _DatasetType
) -> None:
    whole = _loader(dataset_type, DataSource(source_dataset))
    selection = (
        source_dataset.filter_segments("a").filter_contents(["/value/**", "/other/**"]).filter_contents("/value/**")
    )
    selected = _loader(dataset_type, DataSource(selection))

    assert _values(whole) == [0, 1, 2, 3, 3, 3, 10, 11, 12, 13, 13, 13]
    assert _values(selected) == [0, 1, 2, 3, 3, 3]
    with pytest.raises(ValueError, match="Field"):
        _loader(dataset_type, DataSource(selection), entity="other")


@pytest.mark.parametrize("dataset_type", [RerunMapDataset, RerunIterableDataset])
@pytest.mark.parametrize(
    "spans,expected",
    [
        ([("a", -2, 5), ("a", 2, 7), ("b", 0, 3), ("unknown", 0, 3)], [0, 1, 2, 3, 3, 3]),
        ([("b", 0, 3), ("unknown", 0, 3)], []),
    ],
    ids=["clipped-intersection", "empty-intersection"],
)
def test_curation_intersects_selection_segments_and_ranges(
    source_dataset: DatasetEntry, dataset_type: _DatasetType, spans: list[tuple[str, int, int]], expected: list[float]
) -> None:
    selection = source_dataset.filter_segments("a").filter_contents("/value/**")
    source = DataSource(selection, curation=_curation(source_dataset, spans))

    dataset = _loader(dataset_type, source)

    assert len(dataset) == len(expected)
    assert _values(dataset) == expected


@pytest.mark.parametrize("dataset_type", [RerunMapDataset, RerunIterableDataset])
def test_curation_requires_matching_dataset_and_timeline(
    source_dataset: DatasetEntry, dataset_type: _DatasetType
) -> None:
    other = source_dataset.catalog.create_dataset("different_source")
    with pytest.raises(ValueError, match=r"[Dd]ataset|identity"):
        _loader(dataset_type, DataSource(source_dataset, curation=_curation(other, [("a", 0, 1)])))

    with pytest.raises(ValueError, match=r"[Tt]imeline|[Ii]ndex"):
        _loader(
            dataset_type, DataSource(source_dataset, curation=_curation(source_dataset, [("a", 0, 1)], index="frame2"))
        )


def test_repeated_segment_filters_never_widen_a_selection(source_dataset: DatasetEntry) -> None:
    selection = source_dataset.filter_segments("a").filter_contents("/value/**")
    source = DataSource(selection).filter_segments(["a", "b"])
    excluded = source.filter_segments(["b"]).filter_segments(["a", "b"])

    assert _values(_loader(RerunMapDataset, source)) == [0, 1, 2, 3, 3, 3]
    assert _values(_loader(RerunMapDataset, excluded)) == []
    assert _values(_loader(RerunMapDataset, source)) == [0, 1, 2, 3, 3, 3]


def test_empty_temporal_intersection_keeps_sampling_type_and_validation(source_dataset: DatasetEntry) -> None:
    curation = _curation(source_dataset, [("a", 0, 1_000_000_000)], index="time", datatype=pa.duration("ns"))
    source = DataSource(source_dataset, segments=["b"], curation=curation)

    sample_index = SampleIndex.build(source, "time", {}, timeline_sampling=FixedRateSampling(rate_hz=1))

    assert sample_index.total_samples == 0
    assert sample_index.ns_dtype == "timedelta64[ns]"
    assert sample_index.ns_per_sample == 1_000_000_000
    with pytest.raises(TypeError, match="requires timeline_sampling=FixedRateSampling"):
        SampleIndex.build(source, "time", {})
    with pytest.raises(ValueError, match="TimeAlignedSampling requires an integer timeline"):
        SampleIndex.build(source, "time", {}, timeline_sampling=TimeAlignedSampling(keyframe_interval=10))


def test_spawned_workers_keep_selection_and_curation_scope(source_dataset: DatasetEntry) -> None:
    selection = (
        source_dataset.filter_segments("a").filter_contents(["/value/**", "/other/**"]).filter_contents("/value/**")
    )
    curation = _curation(source_dataset, [("a", -2, 2), ("b", 0, 3), ("unknown", 0, 3)])
    dataset = _loader(RerunIterableDataset, DataSource(selection, curation=curation))
    loader = torch.utils.data.DataLoader(dataset, batch_size=None, num_workers=2, multiprocessing_context="spawn")

    values = sorted(float(cast("torch.Tensor", sample["value"]).item()) for sample in loader)

    assert values == [0, 1, 2]


@pytest.mark.parametrize("dataset_type", [RerunMapDataset, RerunIterableDataset])
def test_resolved_curation_round_trip_keeps_selection_without_original_view(
    source_dataset: DatasetEntry, tmp_path: Path, dataset_type: _DatasetType
) -> None:
    selection = source_dataset.filter_segments("a").filter_contents("/value/**")
    spans = _curation(source_dataset, [("a", -2, 2), ("b", 0, 3), ("unknown", 0, 3)])

    resolved = Curation.from_source(selection, index="frame", spans=spans)
    exported = resolved.to_arrow()
    path = tmp_path / "selection.parquet"
    resolved.write_parquet(path)

    assert exported.to_pydict() == {"segment_id": ["a"], "start": [0], "end": [2]}
    assert Curation.from_source(selection, index="frame", spans=spans.to_arrow()).to_arrow().equals(exported)
    expected = _values(_loader(dataset_type, DataSource(selection, curation=spans)))
    for restored in (Curation.from_arrow(source_dataset, exported), Curation.from_parquet(source_dataset, path)):
        assert restored.to_arrow().equals(exported, check_metadata=True)
        assert _values(_loader(dataset_type, DataSource(source_dataset, curation=restored))) == expected == [0, 1, 2]
        assert _values(_loader(dataset_type, DataSource(source_dataset, curation=restored), entity="other")) == [
            0,
            1,
            2,
        ]


@pytest.mark.parametrize(
    "index,datatype,start,end",
    [
        ("frame", pa.int64(), 0, 5),
        ("time", pa.duration("ns"), 0, 5_000_000_000),
        ("stamp", pa.timestamp("ns"), 250_000_000, 5_250_000_000),
    ],
)
def test_curation_from_dataset_or_sql_selection_needs_no_annotations(
    source_dataset: DatasetEntry, index: str, datatype: pa.DataType, start: int, end: int
) -> None:
    context = SessionContext()
    context.register_record_batches("segments", [source_dataset.segment_table().collect()])
    selected_ids = context.sql("SELECT rerun_segment_id FROM segments WHERE rerun_segment_id = 'b'")
    selection = source_dataset.filter_segments(selected_ids)

    whole = Curation.from_source(source_dataset, index=index)
    selected = Curation.from_source(selection, index=index)
    empty = Curation.from_source(selection.filter_segments([]), index=index)

    assert whole.to_arrow()["segment_id"].to_pylist() == ["a", "b"]
    assert selected.to_arrow()["segment_id"].to_pylist() == ["b"]
    assert selected.to_arrow()["start"].cast(pa.int64()).to_pylist() == [start]
    assert selected.to_arrow()["end"].cast(pa.int64()).to_pylist() == [end]
    assert empty.to_arrow().num_rows == 0
    assert empty.to_arrow().schema == selected.to_arrow().schema
    for curation in (whole, selected, empty):
        assert curation.dataset_id == str(source_dataset.id)
        assert curation.index_name == index
        assert curation.index_type == datatype


def test_resolved_curation_export_does_not_bake_in_sampling_or_field_windows(source_dataset: DatasetEntry) -> None:
    spans = _curation(source_dataset, [("a", 1_250_000_000, 3_750_000_000)], index="time", datatype=pa.duration("ns"))
    batch = spans.to_arrow().to_batches()[0]
    resolved = Curation.from_source(source_dataset.filter_segments("a"), index="time", spans=batch)
    exported = resolved.to_arrow()
    fields = {"value": Field("/value:Scalars:scalars", decode=NumericDecoder(), window=(-0.5, 0, 0.5))}

    windowed = SampleIndex.from_curation(resolved, fields, timeline_sampling=FixedRateSampling(rate_hz=1))
    finer = SampleIndex.from_curation(resolved, {}, timeline_sampling=FixedRateSampling(rate_hz=2))

    assert [_index_value_to_int(windowed.global_to_local(i)[1]) for i in range(windowed.total_samples)] == [
        2_000_000_000,
        3_000_000_000,
    ]
    assert [_index_value_to_int(finer.global_to_local(i)[1]) for i in range(finer.total_samples)] == [
        1_500_000_000,
        2_000_000_000,
        2_500_000_000,
        3_000_000_000,
        3_500_000_000,
    ]
    assert resolved.to_arrow().equals(exported, check_metadata=True)
    assert exported["start"].cast(pa.int64()).to_pylist() == [1_250_000_000]
    assert exported["end"].cast(pa.int64()).to_pylist() == [3_750_000_000]
