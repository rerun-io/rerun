"""Reconstruct fetch chunks and decode targets from a manifest's frozen rows."""

from __future__ import annotations

from typing import TYPE_CHECKING

from .._sample_index import SegmentMetadata, _ns_to_dtype
from .._utils import Target, is_video_field
from ._manifest import COL_ANCHOR, COL_SEGMENT_ID, RANGE_LO

if TYPE_CHECKING:
    import pyarrow as pa

    from .._config import Field
    from .._decoders import ColumnDecoder


def targets_from_rows(
    rows: pa.Table,
    *,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    ns_dtype: str | None,
) -> list[Target]:
    """
    Build decode targets from manifest rows, without a keyframe scan.

    A video field's stored `lo` is the prior keyframe the build resolved, so it is reused
    as the decode anchor; query construction and decode are then shared with the live path.
    """
    seg_ids = rows.column(COL_SEGMENT_ID).to_pylist()
    anchors = rows.column(COL_ANCHOR).to_pylist()
    video_los = {
        key: rows.column(key).combine_chunks().field(RANGE_LO).to_pylist()
        for key, field in fields.items()
        if is_video_field(field, decoders[key])
    }
    targets: list[Target] = []
    for i, (segment_id, anchor) in enumerate(zip(seg_ids, anchors, strict=True)):
        prior_keyframes = {key: int(los[i]) for key, los in video_los.items()}
        targets.append(
            Target(
                segment=SegmentMetadata(segment_id=segment_id, index_start=0, index_end=0, num_samples=0),
                index_value=_ns_to_dtype(int(anchor), ns_dtype),
                anchors=prior_keyframes,
            )
        )
    return targets
