"""User-facing configuration dataclasses for catalog-server-backed Torch datasets."""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from rerun.catalog._entry import DatasetEntry, DatasetSelection
    from rerun.chunk._selector import Selector
    from rerun.experimental.spans import Curation

    from .decoders._base import ColumnDecoder, DecodedValue


@dataclass(frozen=True)
class TimeAlignedSampling:
    """
    Sample aligned fields on a shared integer frame grid using exact-index reads.

    Data requirements:

    1. **An integer timeline.** The selected timeline uses integer frame numbers.
    2. **Dense, zero-based indices.** Every selected field (scalars, state, actions,
       video) has one value or frame at each index `0, 1, 2, …`, with no gaps or null values.
    3. **Known video keyframes.** With `keyframe_interval=K`, frames `0, K, 2K, …`
       are valid decode starting points; for `K=10`, frame 27 decodes from frame 20.
    4. **A shared frame-number reference.** The same index refers to the same
       physical instant across fields. The declared keyframe schedule applies
       to every selected video and recording.
    5. **Integer field-window offsets.** Windows use whole-frame steps, such as `(-2, -1, 0)`.

    Decode starts are calculated from the declared interval without an upfront keyframe scan.
    Keyframe markers are fetched with the video data and checked at each expected decode start.
    A missing or false marker leaves that sample's video field unresolved; live iterable datasets
    apply their incomplete-sample skip policy, while map-dataset consumers receive `None`.

    Parameters
    ----------
    keyframe_interval
        Positive number of frames between video keyframes.

    Examples
    --------
    ```python
    dataset = RerunIterableDataset(
        source,
        index="frame_nr",
        fields=fields,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
    )
    ```

    """

    keyframe_interval: int

    def __post_init__(self) -> None:
        if isinstance(self.keyframe_interval, bool) or not isinstance(self.keyframe_interval, int):
            raise ValueError("TimeAlignedSampling.keyframe_interval must be a positive integer")
        if self.keyframe_interval <= 0:
            raise ValueError("TimeAlignedSampling.keyframe_interval must be a positive integer")


@dataclass(frozen=True)
class FixedRateSampling:
    """
    Sample a timestamp or duration timeline on a fixed grid at `rate_hz` samples per second.

    Live grids start at each recording's first time that accommodates all field windows.
    Curated grids start at timeline zero.
    Spacing is rounded to nanoseconds.
    Samples use the latest available values at or before each requested time;
    field windows remain in seconds.
    """

    rate_hz: float

    def __post_init__(self) -> None:
        if not math.isfinite(self.rate_hz) or not 0 < self.rate_hz <= 1e9:
            raise ValueError(
                "FixedRateSampling.rate_hz must be finite, positive, and at most 1e9 (nanosecond resolution)"
            )


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
        Optional jq-like [`Selector`][rerun.chunk.Selector] applied
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

    """

    path: str
    decode: ColumnDecoder[DecodedValue]
    select: Selector | None = None
    window: tuple[int | float, ...] | None = None

    def __post_init__(self) -> None:
        if self.window is not None and not self.window:
            raise ValueError("Field.window must contain at least one offset")
        if self.window is not None and any(not math.isfinite(offset) for offset in self.window):
            raise ValueError(f"Field.window offsets must be finite, got {self.window!r}")

    @property
    def prior_keyframe_path(self) -> str | None:
        """Component path containing the keyframe markers required by this field's decoder."""
        return self.decode.prior_keyframe_path(self.path)

    @property
    def fill_latest_at(self) -> bool:
        """Whether server queries for this field use latest-at filling."""
        return self.decode.fill_latest_at


@dataclass(frozen=True, init=False)
class DataSource:
    """
    A dataset or filtered selection, optionally restricted to curated intervals.

    Parameters
    ----------
    dataset
        The remote dataset or selection to read from.
    segments
        Optional segment IDs, intersected with any segment filter on the selection.
    curation
        Optional curated intervals, intersected with the selected recordings and their timeline ranges.

    """

    dataset: DatasetEntry
    segments: list[str] | None = None
    curation: Curation | None = None
    _content_filters: tuple[str, ...] = field(default=(), init=False, repr=False)

    def __init__(
        self,
        dataset: DatasetEntry | DatasetSelection,
        segments: list[str] | None = None,
        *,
        curation: Curation | None = None,
    ) -> None:
        from rerun.catalog import DatasetSelection

        content_filters: tuple[str, ...] = ()
        if isinstance(dataset, DatasetSelection):
            view_segments = dataset._internal.filtered_segment_ids
            if view_segments is not None:
                segments = sorted(view_segments if segments is None else view_segments.intersection(segments))
            expressions = dataset._internal.content_filters
            content_filters = () if expressions is None else tuple(expressions)
            dataset = dataset.dataset
        object.__setattr__(self, "dataset", dataset)
        object.__setattr__(self, "segments", None if segments is None else list(segments))
        object.__setattr__(self, "curation", curation)
        object.__setattr__(self, "_content_filters", content_filters)

    def _selection(self, *, include_segments: bool = True) -> DatasetEntry | DatasetSelection:
        selection: DatasetEntry | DatasetSelection = self.dataset
        for expression in self._content_filters:
            selection = selection.filter_contents(expression)
        if include_segments and self.segments is not None:
            selection = selection.filter_segments(self.segments)
        return selection

    def filter_segments(self, segment_ids: list[str]) -> DataSource:
        """Return a new DataSource narrowed to `segment_ids`, preserving its contents and curation."""
        segments = segment_ids
        if self.segments is not None:
            allowed = set(self.segments)
            segments = [segment_id for segment_id in segment_ids if segment_id in allowed]
        return DataSource(self._selection(include_segments=False), segments=segments, curation=self.curation)

    def _with_curation(self, curation: Curation) -> DataSource:
        if self.curation is not None:
            raise ValueError("The source already has a curation")
        return DataSource(self._selection(include_segments=False), segments=self.segments, curation=curation)
