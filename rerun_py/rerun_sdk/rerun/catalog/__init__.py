from __future__ import annotations

from typing import Any

from rerun_bindings import (
    AlreadyExistsError as AlreadyExistsError,
    ComponentColumnDescriptor as ComponentColumnDescriptor,
    ComponentColumnSelector as ComponentColumnSelector,
    EntryId as EntryId,
    EntryKind as EntryKind,
    IndexColumnDescriptor as IndexColumnDescriptor,
    IndexColumnSelector as IndexColumnSelector,
    NotFoundError as NotFoundError,
)
from rerun_bindings.types import (
    IndexValuesLike as IndexValuesLike,
)

from ._catalog_client import CatalogClient as CatalogClient, VersionInfo as VersionInfo
from ._content_filter import ContentFilter as ContentFilter
from ._dataset_info import (
    DatasetInfo as DatasetInfo,
    LayerInfo as LayerInfo,
    TimelineInfo as TimelineInfo,
)
from ._entry import (
    DatasetEntry as DatasetEntry,
    DatasetSelection as DatasetSelection,
    Entry as Entry,
    OnDuplicateSegmentLayer as OnDuplicateSegmentLayer,
    TableEntry as TableEntry,
)
from ._layer import Layer as Layer
from ._registration_handle import (
    RegistrationHandle as RegistrationHandle,
    RegistrationResult as RegistrationResult,
    SegmentRegistrationResult as SegmentRegistrationResult,
)
from ._schema import Schema as Schema
from ._segment import Segment as Segment
from ._unregistration_handle import (
    UnregistrationHandle as UnregistrationHandle,
)


def __getattr__(name: str) -> Any:
    """Lazy module-level attribute lookup for deprecated aliases (see PEP 562)."""
    # TODO(RR-6017): remove the `DatasetView` alias.
    if name == "DatasetView":
        import warnings

        warnings.warn(
            "`rerun.catalog.DatasetView` was renamed to `rerun.catalog.DatasetSelection`. "
            "The `DatasetView` alias is deprecated and will be removed in a future release.",
            DeprecationWarning,
            stacklevel=2,
        )
        return DatasetSelection
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
