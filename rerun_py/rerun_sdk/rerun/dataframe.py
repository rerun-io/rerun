from __future__ import annotations

from collections import defaultdict
from typing import TYPE_CHECKING, Any

import pyarrow as pa

from ._send_dataframe import (
    send_record_batch as send_record_batch,  # for backward compatibility
    send_dataframe as send_dataframe,  # for backward compatibility
)
from .catalog import Schema as Schema  # for backward compatibility
from rerun_bindings import (
    ComponentColumnDescriptor as ComponentColumnDescriptor,  # for backward compatibility
    ComponentColumnSelector as ComponentColumnSelector,  # for backward compatibility
    IndexColumnDescriptor as IndexColumnDescriptor,  # for backward compatibility
    IndexColumnSelector as IndexColumnSelector,  # for backward compatibility
    Recording as Recording,
    RecordingView as RecordingView,
    RRDArchive as RRDArchive,
    load_archive as load_archive,
    load_recording as load_recording,
)
from rerun_bindings.types import (
    AnyColumn as AnyColumn,
    AnyComponentColumn as AnyComponentColumn,
    ViewContentsLike as ViewContentsLike,
)
