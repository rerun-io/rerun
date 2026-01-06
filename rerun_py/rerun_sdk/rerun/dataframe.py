"""
Deprecated dataframe module.

.. deprecated::
    This module is deprecated. Use ``rerun.recording`` for loading recordings,
    ``rerun.catalog`` for schema types, and the catalog API for querying data.
    See: https://rerun.io/docs/reference/migration/migration-0-28#recordingview-and-local-dataframe-api-deprecated
"""

from __future__ import annotations

import warnings
from typing import Any

# TODO(RR-3130): this entire submodule is deprecated and will be removed in a future release

_MIGRATION_GUIDE = (
    "https://rerun.io/docs/reference/migration/migration-0-28#recordingview-and-local-dataframe-api-deprecated"
)

_MOVED_TO_RECORDING = {"Recording", "RRDArchive", "load_archive", "load_recording"}
_MOVED_TO_CATALOG = {
    "Schema",
    "ComponentColumnDescriptor",
    "ComponentColumnSelector",
    "IndexColumnDescriptor",
    "IndexColumnSelector",
}
_MOVED_TO_TOP_LEVEL = {"send_dataframe", "send_record_batch"}
_DEPRECATED_QUERY_API = {"RecordingView", "AnyColumn", "AnyComponentColumn", "ViewContentsLike"}


def _get_deprecated_attr(name: str) -> Any:
    """Lazily import and return the deprecated attribute."""
    if name == "Recording":
        from .recording import Recording

        return Recording
    if name == "RRDArchive":
        from .recording import RRDArchive

        return RRDArchive
    if name == "load_archive":
        from .recording import load_archive

        return load_archive
    if name == "load_recording":
        from .recording import load_recording

        return load_recording
    if name == "Schema":
        from .catalog import Schema

        return Schema
    if name == "ComponentColumnDescriptor":
        from rerun_bindings import ComponentColumnDescriptor

        return ComponentColumnDescriptor
    if name == "ComponentColumnSelector":
        from rerun_bindings import ComponentColumnSelector

        return ComponentColumnSelector
    if name == "IndexColumnDescriptor":
        from rerun_bindings import IndexColumnDescriptor

        return IndexColumnDescriptor
    if name == "IndexColumnSelector":
        from rerun_bindings import IndexColumnSelector

        return IndexColumnSelector
    if name == "send_dataframe":
        from ._send_dataframe import send_dataframe

        return send_dataframe
    if name == "send_record_batch":
        from ._send_dataframe import send_record_batch

        return send_record_batch
    if name == "RecordingView":
        from rerun_bindings import RecordingView

        return RecordingView
    if name == "AnyColumn":
        from rerun_bindings.types import AnyColumn

        return AnyColumn
    if name == "AnyComponentColumn":
        from rerun_bindings.types import AnyComponentColumn

        return AnyComponentColumn
    if name == "ViewContentsLike":
        from rerun_bindings.types import ViewContentsLike

        return ViewContentsLike
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


def __getattr__(name: str) -> Any:
    if name in _MOVED_TO_RECORDING:
        warnings.warn(
            f"`rerun.dataframe.{name}` is deprecated. Use `rerun.recording.{name}` instead. See: {_MIGRATION_GUIDE}",
            DeprecationWarning,
            stacklevel=2,
        )
        return _get_deprecated_attr(name)
    if name in _MOVED_TO_CATALOG:
        warnings.warn(
            f"`rerun.dataframe.{name}` is deprecated. Use `rerun.catalog.{name}` instead. See: {_MIGRATION_GUIDE}",
            DeprecationWarning,
            stacklevel=2,
        )
        return _get_deprecated_attr(name)
    if name in _MOVED_TO_TOP_LEVEL:
        warnings.warn(
            f"`rerun.dataframe.{name}` is deprecated. Use `rerun.{name}` instead. See: {_MIGRATION_GUIDE}",
            DeprecationWarning,
            stacklevel=2,
        )
        return _get_deprecated_attr(name)
    if name in _DEPRECATED_QUERY_API:
        warnings.warn(
            f"`rerun.dataframe.{name}` is deprecated. Use the catalog API instead. See: {_MIGRATION_GUIDE}",
            DeprecationWarning,
            stacklevel=2,
        )
        return _get_deprecated_attr(name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


def __dir__() -> list[str]:
    # Include deprecated names in dir() for discoverability
    return list(_MOVED_TO_RECORDING | _MOVED_TO_CATALOG | _MOVED_TO_TOP_LEVEL | _DEPRECATED_QUERY_API)
