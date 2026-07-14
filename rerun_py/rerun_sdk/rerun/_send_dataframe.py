from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import pyarrow as pa

    from .experimental._chunk import DataframeLike
    from .recording_stream import RecordingStream


class _AutoIndex:
    """Sentinel for the `index=…` argument: derive index columns from metadata."""


AUTO_INDEX = _AutoIndex()
"""Sentinel for the `index=…` argument: derive index columns from metadata."""

# The following constants mirror the Rerun Arrow metadata keys (see `re_sorbet::metadata`). They are
# kept here for backwards compatibility; the dataframe → chunk interpretation now lives in Rust.
SORBET_INDEX_NAME = b"rerun:index_name"
SORBET_ENTITY_PATH = b"rerun:entity_path"
SORBET_ARCHETYPE_NAME = b"rerun:archetype"
SORBET_COMPONENT = b"rerun:component"
SORBET_COMPONENT_TYPE = b"rerun:component_type"
SORBET_IS_TABLE_INDEX = b"rerun:is_table_index"
RERUN_KIND = b"rerun:kind"
RERUN_KIND_CONTROL = b"control"
RERUN_KIND_INDEX = b"index"

# Root entity path used for recording-scope properties (e.g. `start_time`).
# Mirrors `re_log_types::EntityPath::properties()` on the Rust side.
RECORDING_PROPERTIES_PATH = "/__properties"


def send_record_batch(
    batch: pa.RecordBatch,
    recording: RecordingStream | None = None,
    *,
    index: str | list[str] | None | _AutoIndex = AUTO_INDEX,
    entity_path: str | None = None,
) -> None:
    """
    Coerce a single pyarrow `RecordBatch` to Rerun structure and log it.

    A thin wrapper over [`Chunk.from_record_batch`][rerun.experimental.Chunk.from_record_batch]
    followed by [`send_chunks`][rerun.experimental.send_chunks]. See `Chunk.from_record_batch` for
    the full column-classification semantics and the conditions under which a `ValueError` is raised.

    Parameters
    ----------
    batch:
        The Arrow record batch to interpret.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    index:
        Determines which columns are index (timeline) columns. See
        [`Chunk.from_record_batch`][rerun.experimental.Chunk.from_record_batch] for the full
        semantics. Defaults to deriving the index from the batch's Rerun metadata.
    entity_path:
        Default entity path for component columns that do not otherwise specify one.

    """
    from .experimental._chunk import Chunk
    from .experimental._send_chunks import send_chunks

    chunks = Chunk.from_record_batch(batch, index=index, entity_path=entity_path)
    send_chunks(chunks, recording=recording)  # NOLINT: send_chunks casts the RecordingStream itself


def send_dataframe(
    df: DataframeLike,
    recording: RecordingStream | None = None,
    *,
    index: str | list[str] | None | _AutoIndex = AUTO_INDEX,
    entity_path: str | None = None,
) -> None:
    """
    Coerce a pyarrow `Table` / `RecordBatch` / `RecordBatchReader`, or a datafusion `DataFrame`, to Rerun structure and log it.

    A thin wrapper over [`Chunk.from_dataframe`][rerun.experimental.Chunk.from_dataframe] followed by
    [`send_chunks`][rerun.experimental.send_chunks]. See `Chunk.from_dataframe` for the accepted input
    types, and `Chunk.from_record_batch` for the full column-classification semantics and the
    conditions under which a `ValueError` is raised.

    Parameters
    ----------
    df:
        The dataframe to interpret. Must be a pyarrow `Table`, pyarrow `RecordBatch`, pyarrow
        `RecordBatchReader`, or datafusion `DataFrame` (an optional dependency) — each has a single
        fixed schema.
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    index:
        Determines which columns are index (timeline) columns. See
        [`Chunk.from_record_batch`][rerun.experimental.Chunk.from_record_batch] for the full
        semantics. Defaults to deriving the index from the dataframe's Rerun metadata.
    entity_path:
        Default entity path for component columns that do not otherwise specify one.

    """
    from .experimental._chunk import Chunk
    from .experimental._send_chunks import send_chunks

    chunks = Chunk.from_dataframe(df, index=index, entity_path=entity_path)
    send_chunks(chunks, recording=recording)  # NOLINT: send_chunks casts the RecordingStream itself
