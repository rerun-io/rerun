from __future__ import annotations

from rerun_bindings import (
    AlreadyExistsError as AlreadyExistsError,
    DataframeQueryView as DataframeQueryView,
    DataFusionTable as DataFusionTable,
    EntryId as EntryId,
    EntryKind as EntryKind,
    IndexConfig as IndexConfig,
    IndexingResult as IndexingResult,
    NotFoundError as NotFoundError,
    Schema as Schema,
    TableInsertMode as TableInsertMode,
    Task as Task,
    Tasks as Tasks,
    VectorDistanceMetric as VectorDistanceMetric,
)

try:
    from rerun_bindings import _rerun_trace_context as _rerun_trace_context  # type: ignore[attr-defined]
except ImportError:
    # perf_telemetry feature not enabled
    _rerun_trace_context = None  # type: ignore[assignment]
from rerun_bindings.types import (
    IndexValuesLike as IndexValuesLike,
    VectorDistanceMetricLike as VectorDistanceMetricLike,
)

from ._catalog_client import CatalogClient as CatalogClient
from ._entry import DatasetEntry as DatasetEntry, Entry as Entry, TableEntry as TableEntry
