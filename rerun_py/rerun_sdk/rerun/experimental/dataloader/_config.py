"""User-facing configuration dataclasses for catalog-server-backed Torch datasets."""

from __future__ import annotations

import math
from dataclasses import dataclass, replace
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from rerun.catalog._entry import DatasetEntry
    from rerun.experimental._selector import Selector

    from .decoders._base import ColumnDecoder, DecodedValue


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
        that turns the Arrow column into a training value.
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
        Optional explicit offsets of the values to return relative to the
        current index value. Integer timelines require integral index-step
        offsets. Timestamp and duration timelines use seconds, which are
        converted to nanoseconds internally.
        Unlike [`FixedRateSampling`][rerun.experimental.dataloader.FixedRateSampling],
        these offsets do not define or interpolate a grid. For example,
        `(-2.5, 0.0)` on a timestamp timeline requests exactly the values at
        2.5 seconds before the current sample and at the current sample.

        An RGB compressed-video window yields a `[T, C, H, W]` frame stack;
        `VideoFrameDecoder(output_format="yuv420p")` instead yields a
        [`Yuv420Frame`][rerun.experimental.dataloader.Yuv420Frame]. Both are bootstrapped from the keyframe preceding
        the earliest output.
    max_staleness
        Optional maximum age of the data backing a sample, using the same unit
        convention as `window`: integral index steps for integer timelines and
        seconds for timestamp or duration timelines. When set, a required
        sample is dropped if the nearest value at or before a queried point is
        older than this. `None` (the default) applies no staleness limit.
        Enforced only during manifest construction, not by the streaming
        dataloader.

        For compressed video, manifests conservatively measure age from the
        latest prior keyframe because sparse keyframe metadata does not expose
        non-keyframe timestamps. A sample may therefore be dropped even when a
        fresher non-keyframe exists.

    """

    path: str
    decode: ColumnDecoder[DecodedValue]
    select: Selector | None = None
    window: tuple[int | float, ...] | None = None
    max_staleness: int | float | None = None

    def __post_init__(self) -> None:
        if self.window is not None and not self.window:
            raise ValueError("Field.window must contain at least one offset")
        if self.window is not None and any(not math.isfinite(offset) for offset in self.window):
            raise ValueError(f"Field.window offsets must be finite, got {self.window!r}")
        if self.max_staleness is not None:
            if not math.isfinite(self.max_staleness):
                raise ValueError(f"Field.max_staleness must be finite, got {self.max_staleness!r}")
            if self.max_staleness < 0:
                raise ValueError(f"Field.max_staleness must be non-negative, got {self.max_staleness!r}")

    @property
    def prior_keyframe_path(self) -> str | None:
        """Component path containing the keyframe markers required by this field's decoder."""
        return self.decode.prior_keyframe_path(self.path)

    @property
    def fill_latest_at(self) -> bool:
        """Whether server queries for this field use latest-at filling."""
        return self.decode.fill_latest_at

    def to_recipe(self) -> dict[str, Any]:
        """
        A JSON-serializable snapshot of this field's spec, for a manifest's provenance header.

        Kept on `Field` so it stays in sync as the spec evolves. `decode` / `select`
        are captured via `repr` — a human-readable record, not a round-trippable form.
        """
        return {
            "path": self.path,
            "window": list(self.window) if self.window is not None else None,
            "max_staleness": self.max_staleness,
            "decoder": repr(self.decode),
            "select": repr(self.select) if self.select is not None else None,
        }


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
