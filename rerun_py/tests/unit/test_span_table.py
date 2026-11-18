from __future__ import annotations

from datetime import UTC, datetime, timedelta, timezone
from types import SimpleNamespace
from typing import Any
from unittest.mock import Mock

import numpy as np
import pyarrow as pa
import pytest
from datafusion import SessionContext
from rerun.experimental.spans import SpanTable

_DURATION = pa.duration("ns")
_TIMESTAMP = pa.timestamp("ns")


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
    table = Mock(spec=["arrow_schema", "append", "upsert"])
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
            r"Span timeline 'capture_time' is not a duration or timestamp timeline",
        ),
        (_span_schema(), {"capture_time": _TIMESTAMP}, r"but the dataset timeline is timestamp"),
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
            "frame",
            None,
            r"Span timeline 'frame' is not a duration or timestamp timeline of the dataset; candidates are \['capture_time'\]",
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
    epoch = datetime(2026, 1, 1, tzinfo=UTC) if datatype == _TIMESTAMP else timedelta(0)
    values: dict[str, Any] = {"segment_id": "episode_a", "start": epoch, "end": epoch + timedelta(seconds=7)}

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
        (_rows(rerun_span_id=pa.array([""])), r"contain empty strings: \['rerun_span_id'\]"),
        (_rows(**{"capture_time:start": pa.array([-(2**63)], _DURATION)}), r"minimum 64-bit value"),
    ],
)
def test_write_rejects_invalid_rows(rows: pa.Table, message: str) -> None:
    spans, table = _spans()

    with pytest.raises(ValueError, match=message):
        spans.write(rows)

    table.append.assert_not_called()


def test_write_fills_missing_span_ids() -> None:
    spans, table = _spans()

    spans.write(pa.concat_tables([_rows(rerun_span_id=pa.array([None], pa.string())), _rows()]))

    (batch,) = table.append.call_args.args
    generated, kept = batch["rerun_span_id"].to_pylist()
    assert len(generated) == 32, f"expected a generated UUID hex, got {generated!r}"
    assert kept == "a1"
