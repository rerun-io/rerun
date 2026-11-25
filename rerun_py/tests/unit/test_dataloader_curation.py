from __future__ import annotations

import io
import json
import pickle
from datetime import timedelta
from types import SimpleNamespace
from typing import TYPE_CHECKING, cast
from unittest.mock import Mock

import numpy as np
import pyarrow as pa
import pytest
import rerun as rr
import torch
from datafusion import SessionContext, col
from PIL import Image
from rerun.experimental import query_metrics
from rerun.experimental.dataloader import (
    BlockShuffle,
    DataSource,
    Field,
    FixedRateSampling,
    ImageDecoder,
    NoShuffle,
    NumericDecoder,
    RerunIterableDataset,
    RerunMapDataset,
    SampleShuffle,
    _iterable_dataset as iterable_dataset,
)
from rerun.experimental.dataloader._sample_index import SampleIndex
from rerun.experimental.spans import Curation, SpanTable

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import DatasetEntry
    from rerun.experimental.dataloader._shuffle import ShuffleStrategy


def _curation(spans: list[tuple[str, int, int]], *, index_type: pa.DataType | None = None) -> Curation:
    index_type = pa.int64() if index_type is None else index_type
    descriptor: dict[str, str | None] = {"kind": "sequence"}
    if pa.types.is_timestamp(index_type):
        descriptor = {"kind": "timestamp", "timezone": index_type.tz}
    elif pa.types.is_duration(index_type):
        descriptor = {"kind": "duration"}
    table = pa.table(
        {
            "segment_id": pa.array([row[0] for row in spans], type=pa.string()),
            "start": pa.array([row[1] for row in spans], type=index_type),
            "end": pa.array([row[2] for row in spans], type=index_type),
        },
        metadata={
            "manifest_format_version": "3",
            "dataset_id": "id",
            "index_name": "time",
            "index_type": json.dumps(descriptor),
        },
    )
    return Curation.from_arrow(cast("DatasetEntry", SimpleNamespace(id="id")), table)


@pytest.mark.parametrize("index_type", [pa.int64(), pa.duration("ns"), pa.timestamp("ns", "UTC")])
def test_curation_freezes_only_canonical_coordinates(tmp_path: Path, index_type: pa.DataType) -> None:
    curation = _curation([("b", 2, 3), ("a", 0, 1), ("a", 0, 1), ("a", 0, 2)], index_type=index_type)
    table = curation.to_arrow()
    assert curation.num_spans() == 3
    assert table.column_names == ["segment_id", "start", "end"]
    assert table["segment_id"].to_pylist() == ["a", "a", "b"]
    assert table["start"].type == table["end"].type == index_type
    assert table["end"].cast(pa.int64()).to_pylist() == [1, 2, 3]
    path = tmp_path / "curation.parquet"
    curation.write_parquet(path)
    assert Curation.from_parquet(curation.dataset, path).to_arrow().equals(table, check_metadata=True)
    assert _curation([("a", (1 << 63) - 1, (1 << 63) - 1)], index_type=index_type).num_spans() == 1


def test_empty_curation_identity_and_old_format_rejection() -> None:
    curation = _curation([])
    assert curation.num_spans() == SampleIndex.from_curation(curation, {}).total_samples == 0
    with pytest.raises(ValueError, match="regenerate"):
        Curation.from_arrow(
            curation.dataset, curation.to_arrow().replace_schema_metadata({b"manifest_format_version": b"1"})
        )
    other = cast("DatasetEntry", SimpleNamespace(id="other"))
    with pytest.raises(ValueError, match="identity"):
        Curation.from_arrow(other, curation.to_arrow())
    with pytest.raises(ValueError, match=r"bound|start|interval"):
        _curation([("a", 2, 1)])


def test_windows_do_not_trim_selected_anchors() -> None:
    fields = {"value": Field("/value:Scalars:scalars", decode=NumericDecoder(), window=(-3, 0, 3))}
    index = SampleIndex.from_curation(_curation([("a", 10, 14), ("a", 20, 24)]), fields)
    assert [index.global_to_local(i)[1] for i in range(index.total_samples)] == [*range(10, 15), *range(20, 25)]
    assert index.output_index_values(10, fields["value"]) == (7, 10, 13)
    assert index.output_index_values(14, fields["value"]) == (11, 14, 17)
    overlapping = _curation([("a", 0, 5), ("a", 2, 7), ("a", 2, 7)])
    fields = {"value": Field("/value:Scalars:scalars", decode=NumericDecoder(), window=(-1, 0, 1))}
    index = SampleIndex.from_curation(overlapping, fields)
    assert [index.global_to_local(i)[1] for i in range(index.total_samples)] == list(range(8))


def test_temporal_grid_is_independent_of_field_windows() -> None:
    origin = 1_700_000_000_000_000_000
    curation = _curation([("a", origin + 1, origin + 300_000_000)], index_type=pa.timestamp("ns"))
    sampling = FixedRateSampling(rate_hz=10)
    full = SampleIndex.from_curation(curation, {}, timeline_sampling=sampling)
    fields = {"value": Field("/value:Scalars:scalars", decode=NumericDecoder(), window=(-1e-9, 0, 1e-9))}
    windowed = SampleIndex.from_curation(curation, fields, timeline_sampling=sampling)
    expected = [np.datetime64(origin + offset, "ns") for offset in [100_000_000, 200_000_000, 300_000_000]]
    assert [full.global_to_local(i)[1] for i in range(full.total_samples)] == expected
    assert [windowed.global_to_local(i)[1] for i in range(windowed.total_samples)] == expected


@pytest.mark.parametrize(
    "index_type", [pa.int64(), pa.timestamp("ns"), pa.duration("ns")], ids=["sequence", "timestamp", "duration"]
)
@pytest.mark.parametrize("anchor,offset", [(-(1 << 63) + 1, -1), ((1 << 63) - 1, 1)])
def test_curated_windows_reject_unrepresentable_endpoints(index_type: pa.DataType, anchor: int, offset: int) -> None:
    curation = _curation([("a", anchor, anchor)], index_type=index_type)
    sequence = pa.types.is_integer(index_type)
    window = (offset if sequence else offset / 1e9,)
    fields = {"value": Field("/value:Scalars:scalars", decode=NumericDecoder(), window=window)}

    sampling = None if sequence else FixedRateSampling(rate_hz=1e9)
    assert SampleIndex.from_curation(curation, {}, timeline_sampling=sampling).total_samples == 1
    with pytest.raises(ValueError, match=r"Curated field windows.*valid 64-bit"):
        SampleIndex.from_curation(curation, fields, timeline_sampling=sampling)


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
@pytest.mark.parametrize(
    "strategy",
    [
        NoShuffle(),
        SampleShuffle(),
        BlockShuffle(),
        BlockShuffle(buffer_size=3),
        BlockShuffle(buffer_size=3, min_fill=1),
    ],
)
def test_seeded_sharding_covers_every_candidate_and_survives_spawn(
    monkeypatch: pytest.MonkeyPatch, strategy: ShuffleStrategy
) -> None:
    monkeypatch.setattr(iterable_dataset, "_validate_source_schema", lambda *_args, **_kwargs: None)
    entry = Mock(spec=["id", "catalog", "schema", "filter_segments", "get_index_ranges"])
    entry.id = "id"
    entry.catalog = SimpleNamespace(url="rerun+http://fake", _internal=SimpleNamespace(token=None))
    entry.schema.return_value.to_arrow.return_value = pa.schema([("time", pa.int64())])
    entry.schema.return_value.index_columns.return_value = [SimpleNamespace(name="time")]
    entry.filter_segments.return_value = entry
    entry.get_index_ranges.return_value = SessionContext().from_arrow(
        pa.table({"rerun_segment_id": ["a", "b"], "time:start": [0, 100], "time:end": [7, 103]})
    )
    source = DataSource(entry)
    dataset = RerunIterableDataset.from_curation(
        _curation([("a", 0, 7), ("b", 100, 103)]),
        source,
        {"state": Field("/state:Scalars:scalars", decode=NumericDecoder())},
        shuffle_strategy=strategy,
        fetch_block_size=2,
        seed=7,
    )

    def worker_indices() -> np.ndarray:
        work_order = dataset._worker_order()
        blocks = list(work_order.fetch_blocks(fetch_block_size=dataset._fetch_block_size))
        return np.concatenate(blocks) if blocks else np.empty(0, dtype=np.int64)

    indices = []
    for rank in range(2):
        monkeypatch.setattr(iterable_dataset, "_live_ddp_topology", lambda rank=rank: (rank, 2))
        for worker in range(2):
            monkeypatch.setattr(
                torch.utils.data, "get_worker_info", lambda worker=worker: SimpleNamespace(id=worker, num_workers=2)
            )
            order = worker_indices()
            assert np.array_equal(order, worker_indices())
            indices.extend(order.tolist())
    assert sorted(indices) == list(range(12))
    expected_order = worker_indices()
    restored = pickle.loads(pickle.dumps(dataset))
    monkeypatch.setattr(iterable_dataset, "_live_ddp_topology", lambda: None)
    assert restored._ddp_topology() == (1, 2)
    restored_order = restored._worker_order()
    restored_blocks = list(restored_order.fetch_blocks(fetch_block_size=restored._fetch_block_size))
    assert np.array_equal(np.concatenate(restored_blocks), expected_order)


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
def test_span_table_to_curation_loads_images_and_skips_missing_values(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    rrd = tmp_path / "training.rrd"
    images = []
    for frame in range(6):
        buffer = io.BytesIO()
        Image.fromarray(np.full((2, 2, 3), frame, dtype=np.uint8)).save(buffer, format="PNG")
        images.append(buffer.getvalue())
    with rr.RecordingStream("rerun_example_curation", recording_id="episode") as rec:
        rec.save(rrd)
        rec.send_columns(
            "image",
            indexes=[rr.TimeColumn("time", duration=[float(frame) for frame in range(6)])],
            columns=rr.EncodedImage.columns(blob=images),
        )
        rec.send_columns(
            "state",
            indexes=[rr.TimeColumn("time", duration=[2.0, 3.0, 4.0, 5.0])],
            columns=rr.Scalars.columns(scalars=[2, 3, 4, 5]),
        )
    fields = {
        "state": Field("/state:Scalars:scalars", decode=NumericDecoder()),
        "image": Field("/image:EncodedImage:blob", decode=ImageDecoder()),
    }
    sampling = FixedRateSampling(rate_hz=1)
    with rr.server.Server(host="127.0.0.1", datasets={"training": [rrd]}) as server:
        entry = server.client().get_dataset("training")
        annotations = SpanTable.create(entry, index="time", schema=pa.schema([("score", pa.float32())]))
        annotations.append(
            segment_id="episode", start=timedelta(seconds=1), end=timedelta(seconds=3), data={"score": 0.9}
        )
        annotations.append(
            segment_id="episode", start=timedelta(seconds=4), end=timedelta(seconds=5), data={"score": 0.5}
        )
        with query_metrics() as queries:
            selected = annotations.reader().filter(col("score") >= 0.7).to_arrow_table().take([0, 0])
            Curation.save(entry, annotations.to_curation_arrow(selected, index="time"), tag="training-v1")
            curation = Curation.open(entry, tag="training-v1").materialize()
        assert not queries.queries, "Curation authoring must not fetch training fields"
        assert curation.num_spans() == 1
        source = DataSource(entry)
        table = curation.to_arrow()
        sequence = Curation.from_arrow(
            entry,
            table.cast(
                pa.schema(
                    [("segment_id", pa.string()), ("start", pa.int64()), ("end", pa.int64())],
                    metadata={**(table.schema.metadata or {}), b"index_type": b'{"kind":"sequence"}'},
                )
            ),
        )
        with query_metrics() as checks:
            for dataset_type in (RerunMapDataset, RerunIterableDataset):
                with pytest.raises(ValueError, match="Timeline 'missing'"):
                    dataset_type(source, "missing", fields)
                with pytest.raises(ValueError, match="Field 'missing'"):
                    dataset_type.from_curation(
                        curation,
                        source,
                        {"missing": Field("/missing:Scalars:scalars", decode=NumericDecoder())},
                        timeline_sampling=sampling,
                    )
                with pytest.raises(ValueError, match="curation requires"):
                    dataset_type.from_curation(sequence, source, fields)
        assert not checks.queries, "Schema validation must not fetch training fields"
        curated = RerunMapDataset.from_curation(curation, source, fields, timeline_sampling=sampling)
        stream = RerunIterableDataset.from_curation(
            curation, source, fields, timeline_sampling=sampling, shuffle_strategy=BlockShuffle(buffer_size=3), seed=7
        )
        entry.set_name("renamed_training")
        actual = curated.__getitems__([2, 1, 0, 2])
        expected = RerunMapDataset(source, "time", fields, timeline_sampling=sampling).__getitems__([3, 2, 1, 3])
        assert len(curated) == len(stream) == 3
        measurement = curated.metrics.summary()
        assert measurement["fetch_wait_fraction"] > 0
        assert measurement["decode_fraction"] > 0
        for a, b in zip(actual, expected, strict=True):
            image, expected_image = a["image"], b["image"]
            assert isinstance(image, torch.Tensor) and isinstance(expected_image, torch.Tensor)
            assert torch.equal(image, expected_image)
            state, expected_state = a["state"], b["state"]
            if expected_state is None:
                assert state is None
            else:
                assert isinstance(state, torch.Tensor) and isinstance(expected_state, torch.Tensor)
                assert torch.equal(state, expected_state)
        with pytest.warns(RuntimeWarning, match="field 'state' has no value"):
            first = list(stream)
        with pytest.warns(RuntimeWarning, match="field 'state' has no value"):
            second = list(stream)
        assert stream.metrics.summary()["skipped_samples"] == 2
        assert stream.metrics.summary()["decode_fraction"] > 0
        orders = []
        for samples in (first, second):
            values = []
            for sample in samples:
                state = sample["state"]
                assert isinstance(state, torch.Tensor)
                values.append(state.item())
            orders.append(values)
        assert orders[0] == orders[1]
        assert sorted(orders[0]) == [2, 3]
        loader = torch.utils.data.DataLoader(stream, batch_size=None, num_workers=2, multiprocessing_context="spawn")
        assert sorted(cast("torch.Tensor", sample["state"]).item() for sample in loader) == [2, 3]
        empty_shard = RerunIterableDataset.from_curation(
            curation, source, fields, timeline_sampling=sampling, shuffle_strategy=NoShuffle()
        )
        monkeypatch.setattr(empty_shard, "_ddp_topology", lambda: (0, 3))
        with pytest.warns(RuntimeWarning, match="field 'state' has no value"):
            assert list(empty_shard) == []
