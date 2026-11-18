from __future__ import annotations

from datetime import UTC, datetime, timedelta, timezone
from types import SimpleNamespace
from typing import Any, Literal
from unittest.mock import Mock
from uuid import UUID

import numpy as np
import pyarrow as pa
import pytest
from datafusion import SessionContext
from rerun.experimental.spans import SpanTable

_DURATION = pa.duration("ns")
_TIMESTAMP = pa.timestamp("ns")
_SEQUENCE = pa.int64()


def _mock_dataset(timelines: dict[str, pa.DataType]) -> Mock:
    """A dataset with the given timelines and a single segment, `episode_a`."""
    dataset = Mock(spec=["name", "id", "catalog", "schema", "segment_table", "filter_segments"])
    dataset.name = "robot_recordings"
    dataset.schema.return_value.to_arrow.return_value = pa.schema(list(timelines.items()))
    dataset.schema.return_value.index_columns.return_value = [SimpleNamespace(name=name) for name in timelines]
    dataset.filter_segments.side_effect = lambda ids: Mock(
        segment_ids=Mock(return_value=[segment for segment in ids if segment == "episode_a"])
    )
    return dataset


def _span_schema(datatype: pa.DataType = _DURATION) -> pa.Schema:
    return pa.schema([
        ("rerun_segment_id", pa.string()),
        ("rerun_span_id", pa.string()),
        ("capture_time:start", datatype),
        ("capture_time:end", datatype),
    ])


def _spans(datatype: pa.DataType = _DURATION) -> tuple[SpanTable, Mock]:
    """A span table on `capture_time`, and the mocked table entry its writes go to."""
    table = Mock(spec=["arrow_schema", "append", "upsert", "overwrite"])
    table.arrow_schema.return_value = _span_schema(datatype)
    return SpanTable(_mock_dataset({"capture_time": datatype}), table), table


def _rows(**columns: pa.Array) -> pa.Table:
    return pa.table({
        "rerun_segment_id": pa.array(["episode_a"]),
        "rerun_span_id": pa.array(["a1"]),
        "capture_time:start": pa.array([0], _DURATION),
        "capture_time:end": pa.array([1], _DURATION),
        **columns,
    })


@pytest.mark.parametrize(
    "table_schema,timelines,message",
    [
        (
            _span_schema(),
            {"other_time": _DURATION},
            r"Span timeline 'capture_time' is not a timeline of the dataset",
        ),
        (_span_schema(), {"capture_time": _TIMESTAMP}, r"but the dataset timeline is timestamp"),
        (_span_schema(_SEQUENCE), {"capture_time": _DURATION}, r"are int64, but the dataset timeline is duration"),
        (_span_schema(pa.int32()), {"capture_time": pa.int32()}, r"Not a timeline datatype: int32"),
        (_span_schema().remove(1), {"capture_time": _DURATION}, r"requires 'rerun_span_id'"),
        (
            _span_schema().append(pa.field("other:start", _DURATION)).append(pa.field("other:end", _DURATION)),
            {"capture_time": _DURATION},
            r"exactly one '<timeline>:start' and '<timeline>:end' pair",
        ),
    ],
)
def test_wrapping_rejects_tables_that_do_not_match_the_dataset(
    table_schema: pa.Schema, timelines: dict[str, pa.DataType], message: str
) -> None:
    table = Mock(spec=["arrow_schema"])
    table.arrow_schema.return_value = table_schema

    with pytest.raises(ValueError, match=message):
        SpanTable(_mock_dataset(timelines), table)


@pytest.mark.parametrize(
    "index,schema,message",
    [
        (
            "missing",
            None,
            r"Span timeline 'missing' is not a timeline of the dataset; candidates are \['capture_time', 'frame'\]",
        ),
        ("capture_time", pa.schema([("rerun_note", pa.string())]), r"'rerun_note' conflicts"),
        ("capture_time", pa.schema([("note:start", _DURATION)]), r"'note:start' conflicts"),
    ],
)
def test_create_rejects_invalid_columns(index: str, schema: pa.Schema | None, message: str) -> None:
    dataset = _mock_dataset({"capture_time": _DURATION, "frame": pa.int64()})

    with pytest.raises(ValueError, match=message):
        SpanTable.create(dataset, index, schema=schema)

    dataset.catalog.create_table.assert_not_called()


def test_create_on_sequence_timeline() -> None:
    dataset = _mock_dataset({"capture_time": _DURATION, "frame": _SEQUENCE})
    dataset.catalog.create_table.side_effect = lambda _name, schema, _url: Mock(arrow_schema=Mock(return_value=schema))

    spans = SpanTable.create(dataset, "frame", name="spans")

    schema = spans.arrow_schema()
    assert (schema.field("frame:start").type, schema.field("frame:end").type) == (_SEQUENCE, _SEQUENCE)


def test_reader_joins_recording_properties() -> None:
    schema = _span_schema().append(pa.field("property:split", pa.string()))
    spans = pa.table(
        {
            "rerun_segment_id": ["episode_a", "deleted_episode"],
            "rerun_span_id": ["a1", "a2"],
            "capture_time:start": [1, 3],
            "capture_time:end": [2, 4],
            "property:split": ["curated", "unmatched"],
        },
        schema=schema,
    )
    recordings = pa.table({
        "rerun_segment_id": ["episode_a"],
        "property:split": ["train"],
        "property:operator": ["operator_a"],
        "unrelated": [1],
    })
    ctx = SessionContext()
    dataset = _mock_dataset({"capture_time": _DURATION})
    dataset.segment_table.return_value = ctx.from_arrow(recordings)
    table = Mock(spec=["arrow_schema", "reader"])
    table.arrow_schema.return_value = schema
    table.reader.return_value = ctx.from_arrow(spans)

    result = SpanTable(dataset, table).reader().to_arrow_table()

    assert result.drop_columns(["capture_time:start", "capture_time:end"]).to_pylist() == [
        {
            "rerun_segment_id": "episode_a",
            "rerun_span_id": "a1",
            "property:split": "curated",
            "property:operator": "operator_a",
        }
    ], "drops spans on missing segments, keeps span columns over same-named properties, and joins only properties"


@pytest.mark.parametrize(
    "datatype,start,end,expected",
    [
        (_SEQUENCE, 3, 7, [3, 7]),
        (_SEQUENCE, np.int32(-4), np.uint64(2**63 - 1), [-4, 2**63 - 1]),
        (_DURATION, timedelta(seconds=1), timedelta(seconds=2, microseconds=7), [1_000_000_000, 2_000_007_000]),
        (_DURATION, np.timedelta64(3, "s"), np.timedelta64(3_000_000_001, "ns"), [3_000_000_000, 3_000_000_001]),
        (
            _TIMESTAMP,
            datetime(2026, 11, 12, 3, 4, 5, 123_457, tzinfo=UTC),
            datetime(2026, 11, 12, 5, 4, 5, 123_457, tzinfo=timezone(timedelta(hours=2))),
            [1_794_452_645_123_457_000, 1_794_452_645_123_457_000],
        ),
        (
            _TIMESTAMP,
            datetime(2026, 11, 12, 3, 4, 5),
            np.datetime64("2026-11-12T03:04:05.000000001"),
            [1_794_452_645_000_000_000, 1_794_452_645_000_000_001],
        ),
    ],
)
def test_append_converts_boundaries_exactly(datatype: pa.DataType, start: Any, end: Any, expected: list[int]) -> None:
    spans, table = _spans(datatype)

    spans.append(segment_id="episode_a", start=start, end=end)

    (batch,) = table.append.call_args.args
    assert [batch[f"capture_time:{marker}"].cast(pa.int64())[0].as_py() for marker in ("start", "end")] == expected


@pytest.mark.parametrize(
    "datatype,overrides,error,message",
    [
        (_DURATION, {"start": 1, "end": 1}, TypeError, "Boundaries on"),
        (_SEQUENCE, {"start": timedelta(1)}, TypeError, "Boundaries on"),
        (_SEQUENCE, {"start": 1.0}, TypeError, "Boundaries on"),
        (_SEQUENCE, {"start": True}, TypeError, "Boundaries on"),
        (_SEQUENCE, {"start": np.timedelta64(1, "ns")}, TypeError, "Boundaries on"),
        (_SEQUENCE, {"end": 2**63}, ValueError, "not representable as a 64-bit integer"),
        (_SEQUENCE, {"end": np.uint64(2**63)}, ValueError, "not representable as a 64-bit integer"),
        (_TIMESTAMP, {"start": timedelta(1), "end": timedelta(1)}, TypeError, "Boundaries on"),
        (_TIMESTAMP, {"end": np.datetime64("2300-01-01")}, ValueError, "not representable exactly"),
        (_DURATION, {"end": timedelta(days=300 * 365)}, ValueError, "not representable exactly"),
        (_DURATION, {"end": np.timedelta64(1_500, "ps")}, ValueError, "not representable exactly"),
        (_DURATION, {"start": timedelta(seconds=8)}, ValueError, r"capture_time:start <= capture_time:end"),
        (_DURATION, {"segment_id": "typo"}, ValueError, r"Unknown segment IDs .*\['typo'\]"),
        (_DURATION, {"data": {"undeclared": True}}, ValueError, r"Unknown span data columns: \['undeclared'\]"),
        (_DURATION, {"data": {"rerun_note": "x"}}, ValueError, r"cannot override managed columns"),
    ],
)
def test_append_rejects_invalid_spans(
    datatype: pa.DataType, overrides: dict[str, Any], error: type[Exception], message: str
) -> None:
    spans, table = _spans(datatype)
    values: dict[str, Any]
    if datatype == _SEQUENCE:
        values = {"segment_id": "episode_a", "start": 0, "end": 7}
    else:
        epoch = datetime(2026, 1, 1, tzinfo=UTC) if datatype == _TIMESTAMP else timedelta(0)
        values = {"segment_id": "episode_a", "start": epoch, "end": epoch + timedelta(seconds=7)}

    with pytest.raises(error, match=message):
        spans.append(**{**values, **overrides})

    table.append.assert_not_called()


@pytest.mark.parametrize("span_id", [None, ""])
def test_upsert_requires_span_id(span_id: Any) -> None:
    spans, table = _spans()

    with pytest.raises((TypeError, ValueError), match="Span IDs must"):
        spans.upsert(segment_id="episode_a", span_id=span_id, start=timedelta(0), end=timedelta(1))

    table.upsert.assert_not_called()


@pytest.mark.parametrize(
    "rows,message",
    [
        (_rows(rerun_segment_id=pa.array([1])), r"requires 'rerun_segment_id' with datatype string"),
        (_rows(rerun_segment_id=pa.array(["typo"])), r"Unknown segment IDs .*\['typo'\]"),
        (_rows(extra=pa.array([1])), r"Undeclared span data columns: \['extra'\]"),
        (_rows(**{"capture_time:end": pa.array([None], _DURATION)}), r"contain nulls: \['capture_time:end'\]"),
        (_rows().drop_columns(["rerun_span_id"]), r"requires 'rerun_span_id' with datatype string"),
        (_rows(rerun_span_id=pa.array([None], pa.string())), r"contain nulls: \['rerun_span_id'\]"),
        (_rows(rerun_span_id=pa.array([""])), r"contain empty strings: \['rerun_span_id'\]"),
        (_rows(**{"capture_time:start": pa.array([-(2**63)], _DURATION)}), r"minimum 64-bit value"),
    ],
)
def test_write_rejects_invalid_rows(rows: pa.Table, message: str) -> None:
    spans, table = _spans()

    with pytest.raises(ValueError, match=message):
        spans.write(rows, mode="upsert")

    table.upsert.assert_not_called()


def _boundaries(column_type: pa.DataType) -> tuple[pa.Array, pa.Array]:
    return pa.array([3], pa.int64()).cast(column_type), pa.array([7], pa.int64()).cast(column_type)


@pytest.mark.parametrize(
    "table_type,start,end,expected",
    [
        (_SEQUENCE, *_boundaries(_SEQUENCE), [3, 7]),
        (_SEQUENCE, *_boundaries(pa.uint8()), [3, 7]),
        (_DURATION, *_boundaries(_SEQUENCE), [3, 7]),
        (_DURATION, *_boundaries(pa.duration("us")), [3_000, 7_000]),
        (_TIMESTAMP, *_boundaries(pa.timestamp("us", tz="UTC")), [3_000, 7_000]),
        (
            _TIMESTAMP,
            pa.array([1], pa.int32()).cast(pa.date32()),
            pa.array([2], pa.int32()).cast(pa.date32()),
            [86_400 * 10**9, 2 * 86_400 * 10**9],
        ),
        (_SEQUENCE, *(array.dictionary_encode() for array in _boundaries(_SEQUENCE)), [3, 7]),
    ],
)
def test_write_accepts_boundary_columns_of_the_timeline_kind(
    table_type: pa.DataType, start: pa.Array, end: pa.Array, expected: list[int]
) -> None:
    spans, table = _spans(table_type)

    spans.write(_rows(**{"capture_time:start": start, "capture_time:end": end}), mode="upsert")

    (batch,) = table.upsert.call_args.args
    assert [batch[f"capture_time:{marker}"].cast(pa.int64())[0].as_py() for marker in ("start", "end")] == expected


@pytest.mark.parametrize(
    "table_type,column",
    [
        (_SEQUENCE, pa.array([3], pa.int64()).cast(_TIMESTAMP)),
        (_SEQUENCE, pa.array([3], pa.int64()).cast(_TIMESTAMP).dictionary_encode()),
        (_SEQUENCE, pa.array([3], pa.int64()).cast(_DURATION)),
        (_SEQUENCE, pa.array([3.0])),
        (_SEQUENCE, pa.array([True])),
        (_SEQUENCE, pa.array(["3"])),
        (_SEQUENCE, pa.array([3], pa.int64()).cast(pa.time64("ns"))),
        (_DURATION, pa.array([3], pa.int64()).cast(_TIMESTAMP)),
        (_DURATION, pa.array([3.0])),
        (_TIMESTAMP, pa.array([3], pa.int64()).cast(_DURATION)),
        (_TIMESTAMP, pa.array(["2026-01-01T00:00:00"])),
    ],
)
@pytest.mark.parametrize("marker", ["start", "end"])
def test_write_rejects_boundary_columns_of_another_kind(table_type: pa.DataType, column: pa.Array, marker: str) -> None:
    spans, table = _spans(table_type)
    rows = _rows(**{
        "capture_time:start": pa.array([3], pa.int64()),
        "capture_time:end": pa.array([7], pa.int64()),
        f"capture_time:{marker}": column,
    })

    with pytest.raises(ValueError, match=rf"capture_time:{marker} is .*, but the .* timeline 'capture_time' takes"):
        spans.write(rows, mode="upsert")

    table.upsert.assert_not_called()


@pytest.mark.parametrize("mode", ["append", "overwrite"])
def test_write_generates_span_ids(mode: Literal["append", "overwrite"]) -> None:
    spans, table = _spans()
    rows = _rows(rerun_span_id=pa.array([None], pa.string()))

    spans.write(rows, mode=mode)

    (batch,) = getattr(table, mode).call_args.args
    assert UUID(batch["rerun_span_id"][0].as_py()).version == 4


@pytest.mark.parametrize("mode", ["append", "overwrite"])
def test_write_rejects_supplied_span_ids(mode: Literal["append", "overwrite"]) -> None:
    spans, table = _spans()

    with pytest.raises(ValueError, match="Span IDs must be omitted or null"):
        spans.write(_rows(), mode=mode)

    getattr(table, mode).assert_not_called()
