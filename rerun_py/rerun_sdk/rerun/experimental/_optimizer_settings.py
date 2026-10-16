"""Settings objects for the private `_optimized_stream()` API of `LazyStore` and `ChunkStore`."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from collections.abc import Sequence

    from rerun_bindings.types import GopBatchingRuleDict, MergeSplitSettingsDict, OwnChunkRuleDict


@dataclass(frozen=True, kw_only=True)
class _MergeSplitSettings:
    """A merge/split target: a byte target with row guards. `0` disables a row guard."""

    max_bytes: int
    max_rows: int = 0
    max_rows_if_unsorted: int = 0

    def _to_internal(self) -> MergeSplitSettingsDict:
        return {
            "max_bytes": self.max_bytes,
            "max_rows": self.max_rows,
            "max_rows_if_unsorted": self.max_rows_if_unsorted,
        }


@dataclass(frozen=True, kw_only=True)
class _OwnChunkRule:
    """
    A component that always gets a chunk of its own.

    Exactly one of `component_type` (a fully qualified type, e.g. `"rerun.components.IsKeyframe"`,
    matching every column of that type) or `component` (one column by identifier, e.g.
    `"VideoStream:is_keyframe"`, typed or not) must be set.

    `entity_filter` takes entity path filter rules, one per line or one per sequence item
    (`"+ /cams/**"`, `"- /cams/aux"`). `None` applies the rule to every entity, `/__properties`
    included, which a `+ /**` rule does not.

    `merge_split` decides how the chunks this rule produces are rechunked: `"inherit"` uses the
    stream's target, `"passthrough"` emits every slice as-is, a `_MergeSplitSettings` uses a target
    of its own.
    """

    component_type: str | None = None
    component: str | None = None
    entity_filter: str | Sequence[str] | None = None
    merge_split: Literal["inherit", "passthrough"] | _MergeSplitSettings = "inherit"

    def __post_init__(self) -> None:
        if (self.component_type is None) == (self.component is None):
            raise ValueError("exactly one of `component_type` and `component` must be set")

    @classmethod
    def for_type(
        cls,
        component_type: str,
        *,
        entity_filter: str | Sequence[str] | None = None,
        merge_split: Literal["inherit", "passthrough"] | _MergeSplitSettings = "inherit",
    ) -> _OwnChunkRule:
        return cls(component_type=component_type, entity_filter=entity_filter, merge_split=merge_split)

    @classmethod
    def for_column(
        cls,
        component: str,
        *,
        entity_filter: str | Sequence[str] | None = None,
        merge_split: Literal["inherit", "passthrough"] | _MergeSplitSettings = "inherit",
    ) -> _OwnChunkRule:
        return cls(component=component, entity_filter=entity_filter, merge_split=merge_split)

    def _to_internal(self) -> OwnChunkRuleDict:
        entity_filter = self.entity_filter
        if entity_filter is not None and not isinstance(entity_filter, str):
            entity_filter = "\n".join(entity_filter)
        merge_split: Literal["inherit", "passthrough"] | MergeSplitSettingsDict = (
            self.merge_split._to_internal() if isinstance(self.merge_split, _MergeSplitSettings) else self.merge_split
        )
        return {
            "component_type": self.component_type,
            "component": self.component,
            "entity_filter": entity_filter,
            "merge_split": merge_split,
        }


@dataclass(frozen=True, kw_only=True)
class _GopBatchingRule:
    """
    GoP batching of the video streams of the entities the filter matches.

    Output chunks hold the `VideoStream:sample` column (plus `is_keyframe` when `keyframes` is
    `"inline"`), start at a keyframe, and never split a GoP; the entity's other columns are
    rechunked as on any entity. Keyframes are detected from the samples: logged
    `VideoStream:is_keyframe` values are dropped (static ones with a warning) and regenerated, and a
    sample the detector rejects counts as a non-keyframe, with a warning. Each stream comes out with
    its codec as one static `VideoStream:codec` value; temporal codecs that disagree are unsupported
    and fail the whole optimization, so exclude such streams through `entity_filter`. Entities
    without a codec, or whose samples span several timeline sets, are skipped with a warning.

    `entity_filter` follows [`_OwnChunkRule`][rerun.experimental._OwnChunkRule].

    `samples` packs whole GoPs toward the global target (`"inherit"`), one chunk per GoP
    (`"passthrough"`), or toward a target of its own (`_MergeSplitSettings`).

    `keyframes` is `"inline"` to add the column to the sample chunks. Otherwise the column gets
    chunks of its own, so readers can find the samples they need without loading the sample
    chunks: `"own_chunk"` packs them toward the global target (one chunk per stream without one),
    a `_MergeSplitSettings` toward a target of its own.
    """

    entity_filter: str | Sequence[str] | None = None
    samples: Literal["inherit", "passthrough"] | _MergeSplitSettings = "inherit"
    keyframes: Literal["inline", "own_chunk"] | _MergeSplitSettings = "own_chunk"

    def _to_internal(self) -> GopBatchingRuleDict:
        entity_filter = self.entity_filter
        if entity_filter is not None and not isinstance(entity_filter, str):
            entity_filter = "\n".join(entity_filter)
        samples: Literal["inherit", "passthrough"] | MergeSplitSettingsDict = (
            self.samples._to_internal() if isinstance(self.samples, _MergeSplitSettings) else self.samples
        )
        keyframes: Literal["inline", "own_chunk"] | MergeSplitSettingsDict = (
            self.keyframes._to_internal() if isinstance(self.keyframes, _MergeSplitSettings) else self.keyframes
        )
        return {
            "entity_filter": entity_filter,
            "samples": samples,
            "keyframes": keyframes,
        }
