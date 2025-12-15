from __future__ import annotations

from rerun_bindings import (
    ComponentColumnDescriptor as ComponentColumnDescriptor,
    ComponentColumnSelector as ComponentColumnSelector,
    IndexColumnDescriptor as IndexColumnDescriptor,
    IndexColumnSelector as IndexColumnSelector,
    RecordingView as RecordingView,
)
from rerun_bindings.types import (
    AnyColumn as AnyColumn,
    AnyComponentColumn as AnyComponentColumn,
    ViewContentsLike as ViewContentsLike,
)

from ._send_dataframe import (
    send_dataframe as send_dataframe,
    send_record_batch as send_record_batch,
)
from .catalog import Schema as Schema
from .recording import (
    Recording as Recording,
    RRDArchive as RRDArchive,
    load_archive as load_archive,
    load_recording as load_recording,
)


# TODO(RR-3130): this entire submodule is deprecated and will be removed in a future release
