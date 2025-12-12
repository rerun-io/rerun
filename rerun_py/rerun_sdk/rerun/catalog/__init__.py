from __future__ import annotations

from rerun_bindings import (
    AlreadyExistsError as AlreadyExistsError,
    ComponentColumnDescriptor as ComponentColumnDescriptor,
    ComponentColumnSelector as ComponentColumnSelector,
    EntryId as EntryId,
    EntryKind as EntryKind,
    IndexColumnDescriptor as IndexColumnDescriptor,
    IndexColumnSelector as IndexColumnSelector,
    IndexConfig as IndexConfig,
    IndexingResult as IndexingResult,
    NotFoundError as NotFoundError,
    # TODO(RR-3130): remove deprecated TableInsertMode in 0.29 or later
    TableInsertMode as TableInsertMode,
    VectorDistanceMetric as VectorDistanceMetric,
    rerun_trace_context as _rerun_trace_context,
)
from rerun_bindings.types import (
    IndexValuesLike as IndexValuesLike,
    VectorDistanceMetricLike as VectorDistanceMetricLike,
)

from ._catalog_client import CatalogClient as CatalogClient
from ._entry import DatasetEntry as DatasetEntry, DatasetView as DatasetView, Entry as Entry, TableEntry as TableEntry
from ._registration_handle import (
    RegistrationHandle as RegistrationHandle,
    RegistrationResult as RegistrationResult,
    SegmentRegistrationResult as SegmentRegistrationResult,
)
from ._schema import Schema as Schema
