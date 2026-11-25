"""Annotate and query time spans within dataset recordings."""

from __future__ import annotations

import json
from datetime import datetime, timedelta
from hashlib import sha256
from typing import TYPE_CHECKING, Any, Literal, TypeGuard, assert_never, overload
from uuid import uuid4

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq

from rerun._tracing import tracing_scope
from rerun.catalog import TimelineKind
from rerun.time import to_nanos, to_nanos_since_epoch

if TYPE_CHECKING:
    import os
    from collections.abc import Mapping

    import datafusion as dfn

    from rerun.catalog import DatasetEntry, DatasetSelection, TableEntry

_SEGMENT_ID = "rerun_segment_id"
_SPAN_ID = "rerun_span_id"
_IDS = {_SEGMENT_ID: pa.string(), _SPAN_ID: pa.string()}

_BOUNDARY_TYPES: dict[TimelineKind, tuple[type, ...]] = {
    TimelineKind.SEQUENCE: (int, np.integer),
    TimelineKind.DURATION: (timedelta, np.timedelta64),
    TimelineKind.TIMESTAMP: (datetime, np.datetime64),
}

_Boundary = int | np.integer | timedelta | np.timedelta64 | datetime | np.datetime64


def _is_step(value: object) -> TypeGuard[int | np.integer]:
    # `numpy.timedelta64` subclasses `numpy.signedinteger`, and `bool` subclasses `int`.
    return isinstance(value, (int, np.integer)) and not isinstance(value, (bool, np.timedelta64))


_BULK_BOUNDARY_TYPES = {
    TimelineKind.SEQUENCE: "integers",
    TimelineKind.DURATION: "durations, or integers as nanoseconds",
    TimelineKind.TIMESTAMP: "timestamps, dates, or integers as nanoseconds since the Unix epoch",
}


def _source_kind_matches(source: pa.DataType, kind: TimelineKind) -> bool:
    """Whether a bulk boundary column of type `source` holds values of timeline kind `kind`, so casting keeps its meaning."""
    while True:
        if isinstance(source, pa.ExtensionType):
            source = source.storage_type
        elif pa.types.is_dictionary(source) or pa.types.is_run_end_encoded(source):
            source = source.value_type
        else:
            break
    # Integers are raw stored values on every kind; a null-typed column is rejected later as missing boundaries.
    if pa.types.is_integer(source) or pa.types.is_null(source):
        return True
    if pa.types.is_duration(source):
        return kind is TimelineKind.DURATION
    if pa.types.is_timestamp(source) or pa.types.is_date(source):
        return kind is TimelineKind.TIMESTAMP
    return False


def _boundary_columns(index: str) -> tuple[str, str]:
    return f"{index}:start", f"{index}:end"


def _is_boundary(name: str) -> bool:
    return name.endswith((":start", ":end"))


def _is_managed(name: str) -> bool:
    """Whether `name` is reserved for the ID and boundary columns, which data columns may not use."""
    return name.startswith("rerun_") or _is_boundary(name)


def _boundary_value(value: _Boundary) -> int | None:
    """The stored 64-bit value: the step itself on sequence timelines, nanoseconds on the others."""
    if _is_step(value):
        step = int(value)
        if not -(1 << 63) <= step < 1 << 63:
            raise ValueError(f"Span boundary {value!r} is not representable as a 64-bit integer")
        return step
    if isinstance(value, (np.timedelta64, np.datetime64)):
        if np.isnat(value):
            return None
        # Depending on its version, numpy either wraps out-of-range values or raises `OverflowError`, and it
        # truncates sub-nanosecond units without raising.
        try:
            exact = value.astype(f"{type(value).__name__}[ns]").astype(value.dtype) == value
        except OverflowError:
            exact = False
        if not exact:
            raise ValueError(f"Span boundary {value!r} is not representable exactly as 64-bit nanoseconds")
    nanos = int(to_nanos(value)) if isinstance(value, (timedelta, np.timedelta64)) else to_nanos_since_epoch(value)
    if not -(1 << 63) <= nanos < 1 << 63:
        raise ValueError(f"Span boundary {value!r} is not representable exactly as 64-bit nanoseconds")
    return nanos


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
    TimelineKind.from_datatype(datatype)
    if (end_type := schema.field(end).type) != datatype:
        raise ValueError(f"Span boundaries on {index!r} must have matching types; got {datatype} and {end_type}")
    return index, datatype


def _dataset_index_type(dataset: DatasetEntry, index: str) -> pa.DataType:
    schema = dataset.schema()
    fields = schema.to_arrow()
    timelines = {column.name: fields.field(column.name).type for column in schema.index_columns()}
    if index not in timelines:
        raise ValueError(
            f"Span timeline {index!r} is not a timeline of the dataset; candidates are {sorted(timelines)}"
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


def _type_descriptor(datatype: pa.DataType) -> dict[str, str | None]:
    if datatype == pa.int64():
        return {"kind": "sequence"}
    if datatype == pa.duration("ns"):
        return {"kind": "duration"}
    if pa.types.is_timestamp(datatype) and datatype.unit == "ns":
        return {"kind": "timestamp", "timezone": datatype.tz}
    raise ValueError(f"Span boundaries require int64, duration[ns], or timestamp[ns], got {datatype}")


def _curation_arrow(spans: pa.Table | pa.RecordBatch, *, index: str, dataset_id: str) -> pa.Table:
    if isinstance(spans, pa.RecordBatch):
        spans = pa.Table.from_batches([spans])
    if not isinstance(spans, pa.Table):
        raise TypeError("Curated spans must be an Arrow Table or RecordBatch")
    table = spans.select(["rerun_segment_id", f"{index}:start", f"{index}:end"]).rename_columns([
        "segment_id",
        "start",
        "end",
    ])
    datatype = table["start"].type
    descriptor = _type_descriptor(datatype)
    if table["end"].type != datatype or table["segment_id"].type not in (pa.string(), pa.large_string()):
        raise ValueError("Curation coordinates require a string segment ID and matching boundary types")
    if any(column.null_count for column in table.columns):
        raise ValueError("Curation coordinates cannot contain nulls")
    start, end = table["start"].cast(pa.int64()), table["end"].cast(pa.int64())
    if any(
        pc.any(values).as_py()
        for values in [pc.equal(table["segment_id"], ""), pc.equal(start, -(1 << 63)), pc.greater(start, end)]
    ):
        raise ValueError("Curation coordinates require nonempty segment IDs and inclusive start <= end bounds")
    return table.replace_schema_metadata({
        "manifest_format_version": "3",
        "dataset_id": dataset_id,
        "index_name": index,
        "index_type": json.dumps(descriptor, sort_keys=True),
    })


class SpanTable:
    """
    Time span annotations for a dataset.

    Data columns and one timeline are fixed at creation.
    Every row requires inclusive start and end boundaries, stored in the timeline's own datatype:
    `int64` steps for sequence timelines, and nanoseconds for duration and timestamp timelines.

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
        self._index_kind = TimelineKind.from_datatype(self._index_type)
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
        Create an empty table on the timeline `index`; `schema` declares data columns.

        `name` defaults to a name in the catalog's `spans` folder derived from the dataset ID.

        Raises
        ------
        AlreadyExistsError
            If a table named `name` already exists.
        ValueError
            If `index` is not a timeline of the dataset, or `schema` declares a reserved column.

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
        start: int | np.integer,
        end: int | np.integer,
        data: Mapping[str, Any] | None = None,
    ) -> str: ...

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
        Sequence timelines take `int` or `numpy.integer` boundaries, stored as given.
        Duration timelines take `datetime.timedelta` or `numpy.timedelta64` boundaries.
        Timestamp timelines take `datetime.datetime` or `numpy.datetime64` boundaries; naive datetimes are UTC.
        Duration and timestamp values convert exactly to nanoseconds.

        Raises
        ------
        TypeError
            If a boundary type does not match the timeline kind.
        ValueError
            If a boundary does not fit in 64 bits or is not exact in nanoseconds, `start` is after `end`, `data` names
            an undeclared or managed column, or `segment_id` is not in the dataset.

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
        start: int | np.integer,
        end: int | np.integer,
        span_id: str,
        data: Mapping[str, Any] | None = None,
    ) -> str: ...

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
            valid = _is_step(boundary) if self._index_kind is TimelineKind.SEQUENCE else isinstance(boundary, accepted)
            if not valid:
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
            start_column: _boundary_value(start),
            end_column: _boundary_value(end),
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
        Write Arrow rows; overwrite with empty input clears the table.

        Append and overwrite generate UUID4 IDs and reject supplied non-null IDs.
        Upsert requires IDs and replaces entire rows rather than patching columns.

        Raises
        ------
        ValueError
            If a column is undeclared, duplicated, or has the wrong ID type; a boundary column is not an integer or the
            timeline's kind of time; a required column contains nulls; an ID is empty; a start is the minimum 64-bit
            value or after its end; or a `rerun_segment_id` is not in the dataset.

        """
        span_batch = self._span_batch(spans, generate_ids=mode in ("append", "overwrite"))
        match mode:
            case "append":
                self.table.append(span_batch)
            case "upsert":
                self.table.upsert(span_batch)
            case "overwrite":
                self.table.overwrite(span_batch)
            case _:
                assert_never(mode)

    def _span_batch(self, data: pa.Table | pa.RecordBatch, *, generate_ids: bool = False) -> pa.RecordBatch:
        if isinstance(data, pa.RecordBatch):
            data = pa.Table.from_batches([data])
        if not isinstance(data, pa.Table):
            raise TypeError("Span data must be an Arrow Table or RecordBatch")
        if duplicates := sorted({name for name in data.column_names if data.column_names.count(name) > 1}):
            raise ValueError(f"Duplicate span data columns: {duplicates}")
        if generate_ids:
            if _SPAN_ID in data.column_names:
                if data[_SPAN_ID].null_count != data.num_rows:
                    raise ValueError(
                        "Span IDs must be omitted or null for append and overwrite; use upsert to supply IDs"
                    )
                data = data.drop_columns([_SPAN_ID])
            data = data.append_column(_SPAN_ID, pa.array([uuid4().hex for _ in range(data.num_rows)], pa.string()))
        schema, index = self._schema, self._index_name
        for name, datatype in _IDS.items():
            if name not in data.column_names or data[name].type != datatype:
                raise ValueError(f"Span data requires {name!r} with datatype {datatype}")
        if undeclared := sorted(set(data.column_names) - set(schema.names)):
            raise ValueError(f"Undeclared span data columns: {undeclared}")
        for name in _boundary_columns(index):
            if name in data.column_names and not _source_kind_matches(data[name].type, self._index_kind):
                raise ValueError(
                    f"{name} is {data[name].type}, but the {self._index_kind} timeline {index!r} takes "
                    f"{_BULK_BOUNDARY_TYPES[self._index_kind]}"
                )
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

    def to_curation_arrow(self, spans: pa.Table | pa.RecordBatch, *, index: str) -> pa.Table:
        """
        Export selected coordinates with dataset and timeline identity, without source reads.

        Input requires `rerun_segment_id` and the table's typed `<index>:start/end` columns.
        Annotation data is discarded; `Curation` sorts and deduplicates the exported coordinates.
        """
        if index != self._index_name:
            raise ValueError(f"Span table timeline is {self._index_name!r}, got {index!r}")
        table = _curation_arrow(spans, index=index, dataset_id=str(self.dataset.id))
        if table["start"].type != self._index_type:
            raise ValueError(f"Curation boundaries must match the span table timeline type {self._index_type}")
        return table


class Curation:
    """
    A selection of inclusive training intervals, independent of editable span annotations.

    Resolve datasets, views, and selected spans with `from_source()`.
    Create local selections with `from_arrow()` or `from_parquet()`, or save and open tagged selections
    in the catalog. Tags are case-sensitive and scoped to the dataset. Saved selections are ordinary
    tables; direct table APIs can still modify them. Tag lookup relies on their generated names.
    Boundaries select sample anchors; field windows may extend beyond them.
    Dataset contents must remain immutable for reproducible loading.
    """

    def __init__(self, dataset: DatasetEntry, table: TableEntry | pa.Table, *, tag: str | None = None) -> None:
        self._dataset, self._table, self._tag = dataset, table, tag
        self._data = self._curation_data(dataset, table) if isinstance(table, pa.Table) else None
        self._schema = self._data.schema if self._data is not None else self.table.arrow_schema()
        if self._data is not None:
            self._table = self._data
            if tag is None:
                encoded_tag = (self._data.schema.metadata or {}).get(b"rerun:curation:tag")
                self._tag = encoded_tag.decode() if encoded_tag is not None else None

    def __repr__(self) -> str:
        return f"Curation(dataset_id={self.dataset_id!r}, tag={self.tag!r})"

    @property
    def dataset(self) -> DatasetEntry:
        """The dataset whose recordings the selection references."""
        return self._dataset

    @property
    def table(self) -> TableEntry:
        """The underlying catalog table; local Arrow and Parquet selections have no catalog entry."""
        if isinstance(self._table, pa.Table):
            raise ValueError("Local curation has no catalog table; save it first")
        return self._table

    @property
    def tag(self) -> str | None:
        """The dataset-scoped name, if this selection was saved under a tag."""
        return self._tag

    @property
    def dataset_id(self) -> str:
        """Identity of the source dataset."""
        return str(self.dataset.id)

    @property
    def index_name(self) -> str:
        """Timeline selected by these intervals."""
        return bytes(self._schema.metadata[b"index_name"]).decode()

    @property
    def index_type(self) -> pa.DataType:
        """Integer steps or nanosecond-resolution timestamps/durations."""
        return self._schema.field("start").type

    def num_spans(self) -> int:
        """Number of distinct selected intervals; catalog selections are read to determine this count."""
        return int(self.to_arrow().num_rows)

    @classmethod
    def from_source(
        cls,
        source: DatasetEntry | DatasetSelection,
        *,
        index: str,
        spans: Curation | pa.Table | pa.RecordBatch | None = None,
    ) -> Curation:
        """
        Resolve a dataset or view and optional span selection into a training manifest.

        Reads recording metadata to freeze the selected recordings' inclusive timeline ranges.
        Selected spans are intersected with those ranges; missing recordings and disjoint intervals are omitted.
        The resulting Arrow coordinates can be saved without retaining the source view or annotation query.
        Content filters, training fields, and sampling settings are not stored in the manifest.

        Parameters
        ----------
        source
            Dataset or filtered selection whose recordings are eligible for training.
        index
            Timeline on which to select intervals.
        spans
            Optional curation or canonical Arrow coordinates from `SpanTable.to_curation_arrow()`.
            Dataset and timeline identity must match the source.
            Omit to select the full timeline ranges of the source's recordings.

        Raises
        ------
        ValueError
            The timeline is missing or the selected spans have a different dataset or timeline identity.

        """
        curation, _ = cls._from_source(source, index=index, spans=spans)
        return curation

    @classmethod
    def _from_source(
        cls,
        source: DatasetEntry | DatasetSelection,
        *,
        index: str,
        spans: Curation | pa.Table | pa.RecordBatch | None = None,
    ) -> tuple[Curation, dict[str, int | None]]:
        from rerun.catalog import DatasetSelection

        dataset = source.dataset if isinstance(source, DatasetSelection) else source
        selected = None
        if spans is not None:
            curation = spans if isinstance(spans, Curation) else cls.from_arrow(dataset, spans)
            if curation.dataset_id != str(dataset.id):
                raise ValueError("Curation dataset identity does not match the supplied source")
            if curation.index_name != index:
                raise ValueError(f"Curation timeline is {curation.index_name!r}, got {index!r}")
            selected = curation.to_arrow()
        datatype = _dataset_index_type(dataset, index)
        if selected is not None:
            if datatype != selected["start"].type:
                raise ValueError(
                    f"Timeline {index!r} has type {datatype}, but the curation requires {selected['start'].type}"
                )
            source = source.filter_segments(sorted(set(selected["segment_id"].to_pylist())))

        start_col, end_col = _boundary_columns(index)
        ranges_df = source.get_index_ranges().select(_SEGMENT_ID, start_col, end_col)
        with tracing_scope("ranges_df.to_arrow_table"):
            ranges = ranges_df.to_arrow_table()
        segment_ids = ranges[_SEGMENT_ID].to_pylist()
        starts = ranges[start_col].cast(pa.int64()).to_pylist()
        ends = ranges[end_col].cast(pa.int64()).to_pylist()
        recording_minima = dict(zip(segment_ids, starts, strict=True))
        bounds = {
            segment_id: (start, end)
            for segment_id, start, end in zip(segment_ids, starts, ends, strict=True)
            if start is not None and end is not None
        }
        if selected is None:
            rows = [(segment_id, start, end) for segment_id, (start, end) in bounds.items()]
        else:
            rows = []
            for segment_id, start, end in zip(
                selected["segment_id"].to_pylist(),
                selected["start"].cast(pa.int64()).to_pylist(),
                selected["end"].cast(pa.int64()).to_pylist(),
                strict=True,
            ):
                if segment_id not in bounds:
                    continue
                recording_start, recording_end = bounds[segment_id]
                lo, hi = max(start, recording_start), min(end, recording_end)
                if lo <= hi:
                    rows.append((segment_id, lo, hi))
        coordinates = _curation_arrow(
            pa.table({
                _SEGMENT_ID: pa.array([row[0] for row in rows], type=pa.string()),
                start_col: pa.array([row[1] for row in rows], type=datatype),
                end_col: pa.array([row[2] for row in rows], type=datatype),
            }),
            index=index,
            dataset_id=str(dataset.id),
        )
        return cls.from_arrow(dataset, coordinates), recording_minima

    @classmethod
    def from_arrow(cls, dataset: DatasetEntry, data: pa.Table | pa.RecordBatch) -> Curation:
        """Validate, sort, and deduplicate exported coordinates without querying the dataset."""
        if isinstance(data, pa.RecordBatch):
            data = pa.Table.from_batches([data])
        if not isinstance(data, pa.Table):
            raise TypeError("Curation data must be an Arrow Table or RecordBatch")
        return cls(dataset, data)

    @classmethod
    def from_parquet(cls, dataset: DatasetEntry, path: str | os.PathLike[str]) -> Curation:
        """Read a portable selection together with its dataset and timeline identity."""
        return cls.from_arrow(dataset, pq.read_table(path))

    def write_parquet(self, path: str | os.PathLike[str], *, row_group_size: int = 1 << 16) -> None:
        """Persist selected coordinates and identity metadata as compressed Parquet."""
        pq.write_table(self.to_arrow(), path, compression="zstd", row_group_size=row_group_size)

    def materialize(self) -> Curation:
        """Read once and return a local selection whose subsequent operations need no source reads."""
        if self._data is not None:
            return self
        local = Curation(self.dataset, self.table, tag=self.tag)
        local._data = self.to_arrow()
        return local

    @classmethod
    def save(cls, dataset: DatasetEntry, data: pa.Table | pa.RecordBatch, *, tag: str) -> Curation:
        """
        Save exported coordinates under a new tag without reading annotation data.

        Existing tags raise `AlreadyExistsError`. Creation and writing are separate operations;
        failed writes can leave an incomplete table, which `to_arrow()` rejects.
        """
        name = cls._curation_name(dataset, tag)
        coordinates = cls.from_arrow(dataset, data).to_arrow()
        coordinates = coordinates.replace_schema_metadata({
            **(coordinates.schema.metadata or {}),
            b"rerun:curation:tag": tag.encode(),
            b"rerun:curation:num_rows": str(coordinates.num_rows).encode(),
        })
        table = dataset.catalog.create_table(name, coordinates.schema)
        table.append(coordinates.to_reader())
        return cls(dataset, table, tag=tag)

    @classmethod
    def open(cls, dataset: DatasetEntry, *, tag: str) -> Curation:
        """Open a saved selection without looking up the dataset's annotation table or reading rows."""
        table = dataset.catalog.get_table(cls._curation_name(dataset, tag))
        return cls(dataset, table, tag=tag)

    def to_arrow(self) -> pa.Table:
        """Return canonical coordinates and identity metadata, rejecting incomplete saved writes."""
        if self._data is not None:
            return self._data
        coordinates = self._curation_data(self.dataset, self.table.reader().to_arrow_table())
        metadata = coordinates.schema.metadata or {}
        if self.tag is not None and metadata.get(b"rerun:curation:tag") != self.tag.encode():
            raise ValueError("Saved curation tag does not match the requested tag")
        if b"rerun:curation:num_rows" not in metadata:
            raise ValueError("Saved curation is missing an expected row count")
        return coordinates

    @staticmethod
    def _curation_name(dataset: DatasetEntry, tag: str) -> str:
        if not isinstance(tag, str) or not tag.strip():  # type: ignore[redundant-expr]
            raise ValueError("Curation tags must be nonempty strings")
        return f"curation_{dataset.id}_{sha256(tag.encode()).hexdigest()}".lower()

    @staticmethod
    def _curation_data(dataset: DatasetEntry, data: pa.Table | pa.RecordBatch) -> pa.Table:
        if isinstance(data, pa.RecordBatch):
            data = pa.Table.from_batches([data])
        if not isinstance(data, pa.Table):
            raise TypeError("Curation data must be an Arrow Table or RecordBatch")
        if data.column_names != ["segment_id", "start", "end"]:
            raise ValueError("Curation data requires columns: segment_id, start, end")
        metadata = data.schema.metadata or {}
        if metadata.get(b"manifest_format_version") != b"3":
            raise ValueError(
                "Unsupported curation format; regenerate it with Curation.from_source() or SpanTable.to_curation_arrow()."
            )
        if metadata.get(b"dataset_id") != str(dataset.id).encode():
            raise ValueError("Curation dataset identity does not match this dataset")
        try:
            index = metadata[b"index_name"].decode()
            descriptor = json.loads(metadata[b"index_type"])
        except (KeyError, ValueError) as err:
            raise ValueError("Curation data requires valid timeline identity metadata") from err
        if not index.strip() or any(ord(c) < 32 for c in index):
            raise ValueError("Curation timeline must be a nonempty name without control characters")
        validated = _curation_arrow(
            data.rename_columns(["rerun_segment_id", f"{index}:start", f"{index}:end"]),
            index=index,
            dataset_id=str(dataset.id),
        )
        if descriptor != json.loads(validated.schema.metadata[b"index_type"]):
            raise ValueError("Curation boundary types do not match its timeline identity")
        if b"rerun:curation:num_rows" in metadata:
            try:
                expected_rows = int(metadata[b"rerun:curation:num_rows"])
            except ValueError as err:
                raise ValueError("Saved curation has an invalid expected row count") from err
            if expected_rows < 0 or data.num_rows != expected_rows:
                raise RuntimeError(f"Incomplete curation: expected {expected_rows} rows, found {data.num_rows}")
        rows = sorted(
            set(
                zip(
                    data["segment_id"].to_pylist(),
                    data["start"].cast(pa.int64()).to_pylist(),
                    data["end"].cast(pa.int64()).to_pylist(),
                    strict=True,
                )
            )
        )
        if b"rerun:curation:num_rows" in metadata:
            metadata = {**metadata, b"rerun:curation:num_rows": str(len(rows)).encode()}
        return pa.table(
            {
                "segment_id": pa.array([row[0] for row in rows], type=pa.string()),
                "start": pa.array([row[1] for row in rows], type=data["start"].type),
                "end": pa.array([row[2] for row in rows], type=data["end"].type),
            },
            metadata=metadata,
        )
