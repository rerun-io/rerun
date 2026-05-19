"""User-facing configuration dataclasses for Rerun Data Platform-backed Torch datasets."""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun.catalog._entry import DatasetEntry

    from ._decoders import ColumnDecoder


@dataclass(frozen=True)
class Column:
    """
    Declarative column definition for a training sample.

    Parameters
    ----------
    path
        Entity path + component in the Rerun store,
        e.g. `"/camera:EncodedImage:blob"`.
    decode
        A [`ColumnDecoder`][rerun.experimental.dataloader.ColumnDecoder] instance that converts raw Arrow data
        into a tensor (e.g. `NumericDecoder()` or `ImageDecoder()`).
    window
        Optional `(start_offset, end_offset)` inclusive range relative
        to the current index value. `(0, 99)` means "current frame
        plus the next 99".

    """

    path: str
    decode: ColumnDecoder
    window: tuple[int, int] | None = None


@dataclass(frozen=True)
class DataSource:
    """
    An immutable reference to a dataset with an optional segment filter.

    Parameters
    ----------
    dataset
        The remote dataset to read from.
    segments
        Optional list of segment IDs to restrict to.

    """

    dataset: DatasetEntry
    segments: list[str] | None = None

    def filter_segments(self, segment_ids: list[str]) -> DataSource:
        """Return a new DataSource narrowed to *segment_ids*."""
        return replace(self, segments=segment_ids)
