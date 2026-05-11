"""User-facing configuration dataclasses for Rerun Data Platform-backed Torch datasets."""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun.catalog._entry import DatasetEntry
    from rerun.experimental._selector import Selector

    from ._decoders import ColumnDecoder


@dataclass(frozen=True)
class Field:
    """
    Declarative spec for one field of a training sample.

    !!! note
        This API is provisional and will be improved, expect the surface to change.

    Parameters
    ----------
    path
        `entity_path:Archetype:component` triple identifying the source
        column (e.g. `"/camera:EncodedImage:blob"`).
    decode
        A [`ColumnDecoder`][rerun.experimental.dataloader.ColumnDecoder]
        that turns the Arrow column into a tensor.
    select
        Optional jq-like [`Selector`][rerun.experimental.Selector] applied
        client-side to the Arrow column before `decode`. Used for nested
        struct/list access. The server-side projection is unaffected.

        ```python
        Field(
            path="/agent:ListOfStructs:animals",
            select=Selector(".[0].dog"),
            decode=NumericDecoder(),
        )
        ```
    window
        Optional `(start_offset, end_offset)` range, inclusive on both
        ends and added to the current index value. The field then yields
        the slice of values across that window instead of a single
        sample. Offsets are in the index timeline's native unit:
        integer steps for integer-indexed timelines, nanoseconds for
        timestamp timelines (use multiples of the
        [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling]
        period to align with the sampling grid). For example, `(1, 50)`
        on an integer timeline fetches the next 50 values after the
        current sample.

    """

    path: str
    decode: ColumnDecoder
    select: Selector | None = None
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
