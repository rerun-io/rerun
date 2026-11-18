"""Annotate and query time spans within dataset recordings."""

from __future__ import annotations

from datetime import datetime, timedelta
from typing import TYPE_CHECKING, Any, Literal, assert_never, overload
from uuid import uuid4

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc

from rerun.catalog import TimelineKind
from rerun.time import to_nanos, to_nanos_since_epoch

if TYPE_CHECKING:
    from collections.abc import Mapping

    import datafusion as dfn

    from rerun.catalog import DatasetEntry, TableEntry

_SEGMENT_ID = "rerun_segment_id"
_SPAN_ID = "rerun_span_id"
_IDS = {_SEGMENT_ID: pa.string(), _SPAN_ID: pa.string()}

_BOUNDARY_TYPES: dict[TimelineKind, tuple[type, ...]] = {
    TimelineKind.DURATION: (timedelta, np.timedelta64),
    TimelineKind.TIMESTAMP: (datetime, np.datetime64),
}

_Boundary = timedelta | np.timedelta64 | datetime | np.datetime64


def _boundary_columns(index: str) -> tuple[str, str]:
    return f"{index}:start", f"{index}:end"


def _is_boundary(name: str) -> bool:
    return name.endswith((":start", ":end"))


def _is_managed(name: str) -> bool:
    """Whether `name` is reserved for the ID and boundary columns, which data columns may not use."""
    return name.startswith("rerun_") or _is_boundary(name)


def _boundary_nanos(value: _Boundary) -> int | None:
    if isinstance(value, (np.timedelta64, np.datetime64)):
        if np.isnat(value):
            return None
        # numpy wraps out-of-range values and truncates sub-nanosecond units without raising.
        if value.astype(f"{type(value).__name__}[ns]").astype(value.dtype) != value:
            raise ValueError(f"Span boundary {value!r} is not representable exactly as 64-bit nanoseconds")
    nanos = int(to_nanos(value)) if isinstance(value, (timedelta, np.timedelta64)) else to_nanos_since_epoch(value)
    if not -(1 << 63) <= nanos < 1 << 63:
        raise ValueError(f"Span boundary {value!r} is not representable exactly as 64-bit nanoseconds")
    return nanos


def _time_kind(datatype: pa.DataType) -> TimelineKind:
    kind = TimelineKind.from_datatype(datatype)
    if kind not in _BOUNDARY_TYPES:
        raise ValueError(f"Span boundaries require duration[ns] or timestamp[ns]; got {datatype}")
    return kind


# TODO(RR-6288): `rr.catalog.Schema` should handle that for us.
def _validate_schema(schema: pa.Schema) -> tuple[str, pa.DataType]:
    names = schema.names
    if duplicates := sorted({name for name in names if names.count(name) > 1}):
        raise ValueError(f"Duplicate span columns: {duplicates}")
    if reserved := [
        name for name in names if name.startswith("rerun_") and name not in _IDS and not _is_boundary(name)
    ]:
        raise ValueError(f"Span columns {reserved} use the reserved 'rerun_' prefix")
    for name, datatype in _IDS.items():
        if name not in names or schema.field(name).type != datatype:
            raise ValueError(f"Span table requires {name!r} with datatype {datatype}")
    starts = {name.removesuffix(":start") for name in names if name.endswith(":start")}
    ends = {name.removesuffix(":end") for name in names if name.endswith(":end")}
    if len(starts) != 1 or starts != ends:
        raise ValueError(
            "Span tables require exactly one '<timeline>:start' and '<timeline>:end' pair; "
            f"found starts for {sorted(starts)} and ends for {sorted(ends)}"
        )
    index = next(iter(starts))
    start, end = _boundary_columns(index)
    datatype = schema.field(start).type
    _time_kind(datatype)
    if (end_type := schema.field(end).type) != datatype:
        raise ValueError(f"Span boundaries on {index!r} must have matching types; got {datatype} and {end_type}")
    return index, datatype


def _dataset_index_type(dataset: DatasetEntry, index: str) -> pa.DataType:
    schema = dataset.schema()
    fields = schema.to_arrow()
    timelines = {column.name: fields.field(column.name).type for column in schema.index_columns()}
    if index not in timelines or TimelineKind.from_datatype(timelines[index]) not in _BOUNDARY_TYPES:
        candidates = sorted(
            name for name, datatype in timelines.items() if TimelineKind.from_datatype(datatype) in _BOUNDARY_TYPES
        )
        raise ValueError(
            f"Span timeline {index!r} is not a duration or timestamp timeline of the dataset; candidates are {candidates}"
        )
    return timelines[index]


def _span_schema(data: pa.Schema | None, index: str, index_type: pa.DataType) -> pa.Schema:
    from rerun import SORBET_IS_TABLE_INDEX

    data = pa.schema([]) if data is None else data
    if not isinstance(data, pa.Schema):
        raise TypeError("Span data schema must be an Arrow Schema")
    for field in data:
        if _is_managed(field.name) or (field.metadata or {}).get(SORBET_IS_TABLE_INDEX, b"").lower() == b"true":
            raise ValueError(
                f"Span data column {field.name!r} conflicts with the identity, boundary, and index columns "
                "that are supplied automatically"
            )
    fields = [
        pa.field(
            name,
            datatype,
            nullable=False,
            metadata={SORBET_IS_TABLE_INDEX: b"true"} if name == _SPAN_ID else None,
        )
        for name, datatype in _IDS.items()
    ]
    bounds = [pa.field(name, index_type, nullable=False) for name in _boundary_columns(index)]
    schema = pa.schema([*fields, *bounds, *data], metadata=data.metadata)
    _validate_schema(schema)
    return schema


class SpanTable:
    """
    Time span annotations for a dataset.

    Data columns and one duration or timestamp timeline are fixed at creation.
    Every row requires inclusive start and end boundaries, stored in nanoseconds.
    Sequence timelines are unsupported.

    Span annotations have an independent lifecycle from their dataset.
    """

    def __init__(self, dataset: DatasetEntry, table: TableEntry) -> None:
        """
        Wrap an existing span table; [`SpanTable.open`][rerun.experimental.spans.SpanTable.open] finds a table by name.

        Parameters
        ----------
        dataset
            The dataset whose segments the spans annotate.
        table
            A table with a span schema, as created by [`SpanTable.create`][rerun.experimental.spans.SpanTable.create].

        Raises
        ------
        ValueError
            If `table` is not a span table, or its timeline is not in `dataset` with the same type.

        """
        self._dataset, self._table = dataset, table
        self._schema = table.arrow_schema()
        self._index_name, self._index_type = _validate_schema(self._schema)
        self._index_kind = _time_kind(self._index_type)
        dataset_type = _dataset_index_type(dataset, self._index_name)
        if dataset_type != self._index_type:
            raise ValueError(
                f"Span boundaries on {self._index_name!r} are {self._index_type}, but the dataset timeline is {dataset_type}"
            )

    def __repr__(self) -> str:
        return f"SpanTable(dataset={self._dataset.name!r}, table={self._table.name!r})"

    @classmethod
    def create(
        cls,
        dataset: DatasetEntry,
        index: str,
        *,
        name: str | None = None,
        schema: pa.Schema | None = None,
        url: str | None = None,
    ) -> SpanTable:
        """
        Create an empty table on the duration or timestamp timeline `index`; `schema` declares data columns.

        `name` defaults to a name in the catalog's `spans` folder derived from the dataset ID.

        Raises
        ------
        AlreadyExistsError
            If a table named `name` already exists.
        ValueError
            If `index` is not a duration or timestamp timeline of the dataset, or `schema` declares a reserved column.

        """
        columns = _span_schema(schema, index, _dataset_index_type(dataset, index))
        table = dataset.catalog.create_table(cls._default_table_name(dataset) if name is None else name, columns, url)
        return cls(dataset, table)

    @classmethod
    def open(cls, dataset: DatasetEntry, *, name: str | None = None) -> SpanTable:
        """
        Open a span table of `dataset`.

        `name` defaults to a name in the catalog's `spans` folder derived from the dataset ID.

        Raises
        ------
        LookupError
            If no table named `name` exists.

        """
        return cls(dataset, dataset.catalog.get_table(cls._default_table_name(dataset) if name is None else name))

    @staticmethod
    def _default_table_name(dataset: DatasetEntry) -> str:
        # Derived from the dataset ID so it survives dataset renames; the `spans.` prefix groups span tables in one catalog folder.
        return f"spans.dataset_{dataset.id}".lower()

    @property
    def dataset(self) -> DatasetEntry:
        """The dataset whose recordings the spans select."""
        return self._dataset

    @property
    def table(self) -> TableEntry:
        """The table containing these span annotations."""
        return self._table

    # TODO(RR-6286): we should rely on `rr.catalog.Schema` and a `.schema()` method instead
    def arrow_schema(self) -> pa.Schema:
        """Returns the Arrow schema of the underlying table entry, which is fixed at creation."""
        return self._schema

    def delete(self) -> None:
        """Delete this table without deleting its dataset."""
        self.table.delete()

    @overload
    def append(
        self,
        *,
        segment_id: str,
        start: timedelta | np.timedelta64,
        end: timedelta | np.timedelta64,
        data: Mapping[str, Any] | None = None,
    ) -> str: ...

    @overload
    def append(
        self,
        *,
        segment_id: str,
        start: datetime | np.datetime64,
        end: datetime | np.datetime64,
        data: Mapping[str, Any] | None = None,
    ) -> str: ...

    def append(
        self,
        *,
        segment_id: str,
        start: _Boundary,
        end: _Boundary,
        data: Mapping[str, Any] | None = None,
    ) -> str:
        """
        Append one span and return its generated ID; data values populate declared columns.

        `segment_id` must name a segment of the dataset.
        `start` and `end` are inclusive.
        Duration timelines take `datetime.timedelta` or `numpy.timedelta64` boundaries.
        Timestamp timelines take `datetime.datetime` or `numpy.datetime64` boundaries; naive datetimes are UTC.
        Values convert exactly to nanoseconds.

        Raises
        ------
        TypeError
            If a boundary type does not match the timeline kind.
        ValueError
            If a boundary is not exact in nanoseconds, `start` is after `end`, `data` names an undeclared or managed
            column, or `segment_id` is not in the dataset.

        """
        return self._write_span(
            "append",
            segment_id=segment_id,
            start=start,
            end=end,
            span_id=uuid4().hex,
            data=data,
        )

    @overload
    def upsert(
        self,
        *,
        segment_id: str,
        start: timedelta | np.timedelta64,
        end: timedelta | np.timedelta64,
        span_id: str,
        data: Mapping[str, Any] | None = None,
    ) -> str: ...

    @overload
    def upsert(
        self,
        *,
        segment_id: str,
        start: datetime | np.datetime64,
        end: datetime | np.datetime64,
        span_id: str,
        data: Mapping[str, Any] | None = None,
    ) -> str: ...

    def upsert(
        self,
        *,
        segment_id: str,
        start: _Boundary,
        end: _Boundary,
        span_id: str,
        data: Mapping[str, Any] | None = None,
    ) -> str:
        """
        Insert or fully replace a row by its required ID; omitted nullable data columns become null.

        `start` and `end` follow the same typed boundary rules as [`SpanTable.append`][rerun.experimental.spans.SpanTable.append].

        Raises
        ------
        TypeError
            If `span_id` is not a string, or a boundary type does not match the timeline kind.
        ValueError
            If `span_id` is empty, or for the same invalid spans as [`SpanTable.append`][rerun.experimental.spans.SpanTable.append].

        """
        # Callers without a type checker can pass any value.
        if not isinstance(span_id, str):  # type: ignore[redundant-expr]
            raise TypeError(f"Span IDs must be strings, got {type(span_id).__name__}")
        if not span_id:
            raise ValueError("Span IDs must be nonempty")
        return self._write_span(
            "upsert",
            segment_id=segment_id,
            start=start,
            end=end,
            span_id=span_id,
            data=data,
        )

    def _write_span(
        self,
        mode: Literal["append", "upsert"],
        *,
        segment_id: str,
        start: _Boundary,
        end: _Boundary,
        span_id: str,
        data: Mapping[str, Any] | None,
    ) -> str:
        accepted = _BOUNDARY_TYPES[self._index_kind]
        for boundary in (start, end):
            if not isinstance(boundary, accepted):
                expected = " or ".join(f"{t.__module__}.{t.__name__}" for t in accepted)
                raise TypeError(
                    f"Boundaries on {self._index_kind} timeline {self._index_name!r} must be {expected}, "
                    f"got {type(boundary).__name__}"
                )
        schema = self._schema
        attributes = dict(data or {})
        if managed := sorted(filter(_is_managed, attributes)):
            raise ValueError(f"Span data cannot override managed columns {managed}")
        if unknown := set(attributes) - set(schema.names):
            raise ValueError(f"Unknown span data columns: {sorted(unknown)}")
        start_column, end_column = _boundary_columns(self._index_name)
        values = {
            **attributes,
            _SEGMENT_ID: segment_id,
            _SPAN_ID: span_id,
            start_column: _boundary_nanos(start),
            end_column: _boundary_nanos(end),
        }
        batch = self._span_batch(pa.RecordBatch.from_pylist([values], schema=schema))
        match mode:
            case "append":
                self.table.append(batch)
            case "upsert":
                self.table.upsert(batch)
            case _:
                assert_never(mode)
        return span_id

    def write(
        self, spans: pa.Table | pa.RecordBatch, *, mode: Literal["append", "upsert", "overwrite"] = "append"
    ) -> None:
        """
        Write Arrow rows, generating missing IDs; overwrite with empty input clears the table.

        Caller-supplied IDs must remain unique. Upsert replaces entire rows rather than patching columns.

        Raises
        ------
        ValueError
            If a column is undeclared, duplicated, or has the wrong ID type; a required column contains nulls; an ID is
            empty; a start is the minimum 64-bit value or after its end; or a `rerun_segment_id` is not in the dataset.

        """
        span_batch = self._span_batch(spans)
        match mode:
            case "append":
                self.table.append(span_batch)
            case "upsert":
                self.table.upsert(span_batch)
            case "overwrite":
                self.table.overwrite(span_batch)
            case _:
                assert_never(mode)

    def _span_batch(self, data: pa.Table | pa.RecordBatch) -> pa.RecordBatch:
        if isinstance(data, pa.RecordBatch):
            data = pa.Table.from_batches([data])
        if not isinstance(data, pa.Table):
            raise TypeError("Span data must be an Arrow Table or RecordBatch")
        if _SPAN_ID not in data.column_names:
            data = data.append_column(_SPAN_ID, pa.array([uuid4().hex for _ in range(data.num_rows)], pa.string()))
        elif pa.types.is_null(data[_SPAN_ID].type) or (
            data[_SPAN_ID].type == pa.string() and data[_SPAN_ID].null_count
        ):
            data = data.set_column(
                data.column_names.index(_SPAN_ID),
                _SPAN_ID,
                pa.array(
                    [uuid4().hex if value is None else value for value in data[_SPAN_ID].to_pylist()],
                    pa.string(),
                ),
            )
        schema, index = self._schema, self._index_name
        for name, datatype in _IDS.items():
            if name not in data.column_names or data[name].type != datatype:
                raise ValueError(f"Span data requires {name!r} with datatype {datatype}")
        if duplicates := sorted({name for name in data.column_names if data.column_names.count(name) > 1}):
            raise ValueError(f"Duplicate span data columns: {duplicates}")
        if undeclared := sorted(set(data.column_names) - set(schema.names)):
            raise ValueError(f"Undeclared span data columns: {undeclared}")
        columns = [
            data[field.name].cast(field.type).combine_chunks()
            if field.name in data.column_names
            else pa.nulls(data.num_rows, type=field.type)
            for field in schema
        ]
        batch = pa.RecordBatch.from_arrays(columns, schema=schema)
        if nulls := [
            field.name
            for field, column in zip(schema, columns, strict=True)
            if column.null_count and (not field.nullable or _is_managed(field.name))
        ]:
            raise ValueError(f"Required span columns contain nulls: {nulls}")
        if empty := [name for name in _IDS if pc.any(pc.equal(batch[name], "")).as_py()]:
            raise ValueError(f"Span ID columns contain empty strings: {empty}")
        start_column, end_column = _boundary_columns(index)
        start, end = batch[start_column].cast(pa.int64()), batch[end_column].cast(pa.int64())
        if pc.any(pc.equal(start, -(1 << 63))).as_py():
            raise ValueError(f"{start_column} contains the minimum 64-bit value, which is reserved for missing time")
        if pc.any(pc.greater(start, end)).as_py():
            raise ValueError(f"Spans require {start_column} <= {end_column}")
        self._check_segments(batch[_SEGMENT_ID])
        return batch

    def _check_segments(self, segment_ids: pa.Array) -> None:
        requested = pc.unique(segment_ids).to_pylist()
        if not requested:
            return
        if missing := set(requested) - set(self.dataset.filter_segments(requested).segment_ids()):
            raise ValueError(f"Unknown segment IDs for dataset {self.dataset.name!r}: {sorted(missing)}")

    def reader(self) -> dfn.DataFrame:
        """
        Build a lazy DataFusion query on one timeline, joined with recording properties.

        Filtering selects whole spans without altering boundaries or subtracting overlaps.
        Spans on segments no longer in the dataset are omitted, as are properties named like a span column.
        Low-level table writes remain responsible for valid span rows.
        """
        frame = self.table.reader()
        recordings = self.dataset.segment_table()
        properties = [
            name
            for name in recordings.schema().names
            if name.startswith("property:") and name not in frame.schema().names
        ]
        return frame.alias("spans").join(
            recordings.select(*recordings.find_qualified_columns(_SEGMENT_ID, *properties)).alias("recordings"),
            on=_SEGMENT_ID,
            how="inner",
        )
