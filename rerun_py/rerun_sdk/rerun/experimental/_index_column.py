"""A typed timeline-index specification shared by the experimental readers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

TimeUnit = Literal["ns", "us", "ms", "s"]
"""What the raw index values represent — not a desired output unit."""


@dataclass(frozen=True)
class IndexColumn:
    """
    A dataset/column to use as a timeline index, and how to interpret it.

    Construct one with [`timestamp`][rerun.experimental.IndexColumn.timestamp],
    [`duration`][rerun.experimental.IndexColumn.duration], or
    [`sequence`][rerun.experimental.IndexColumn.sequence] — the timeline kind is
    the constructor you pick, so there is nothing to mistype:

    ```python
    IndexColumn.timestamp("/time", input_unit="s")
    IndexColumn.duration("/elapsed", input_unit="us")
    IndexColumn.sequence("/frame_id")
    ```
    """

    path: str
    """Path of the 1-D dataset (HDF5) or name of the column (Parquet) to use as the index."""

    kind: Literal["timestamp", "duration", "sequence"]
    """The timeline kind: time since epoch, elapsed time, or an ordinal integer index."""

    input_unit: TimeUnit | None = None
    """
    What the raw integer/float values represent (**not** a desired output unit);
    values are scaled to nanoseconds internally. `None` for `sequence`.
    """

    @classmethod
    def timestamp(cls, path: str, *, input_unit: TimeUnit = "ns") -> IndexColumn:
        """A time-since-epoch timeline. `input_unit` describes the raw values (default `"ns"`)."""
        return cls(path=path, kind="timestamp", input_unit=input_unit)

    @classmethod
    def duration(cls, path: str, *, input_unit: TimeUnit = "ns") -> IndexColumn:
        """An elapsed-time timeline. `input_unit` describes the raw values (default `"ns"`)."""
        return cls(path=path, kind="duration", input_unit=input_unit)

    @classmethod
    def sequence(cls, path: str) -> IndexColumn:
        """An ordinal integer-index timeline. No unit applies."""
        return cls(path=path, kind="sequence", input_unit=None)

    def _as_internal_tuple(self) -> tuple[str, str, str | None]:
        """The `(path, kind, unit)` triple the internal bindings expect."""
        return (self.path, self.kind, self.input_unit)
