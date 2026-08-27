"""Reconstruct fetch blocks and decode targets from a manifest's frozen rows."""

from __future__ import annotations

from typing import TYPE_CHECKING

from .._sample_index import SegmentMetadata, _ns_to_dtype
from .._utils import FieldFetchRequest, Target, is_video_field
from ._manifest import COL_ANCHOR, COL_SEGMENT_ID, RANGE_HI, RANGE_LO

if TYPE_CHECKING:
    import pyarrow as pa

    from .._config import Field
    from .._sample_index import SampleIndex


def targets_from_rows(
    rows: pa.Table,
    *,
    fields: dict[str, Field],
    sample_index: SampleIndex,
) -> list[Target]:
    """
    Build decode targets from manifest rows, without a keyframe scan.

    A video field's stored `lo` is the prior keyframe the build resolved, so it is reused
    as the decode anchor; query construction and decode are then shared with the live path.
    """
    seg_ids = rows.column(COL_SEGMENT_ID).to_pylist()
    anchors = rows.column(COL_ANCHOR).to_pylist()
    field_ranges = {
        key: (
            rows.column(key).combine_chunks().field(RANGE_LO).to_pylist(),
            rows.column(key).combine_chunks().field(RANGE_HI).to_pylist(),
        )
        for key in fields
    }
    targets: list[Target] = []
    for i, (segment_id, anchor) in enumerate(zip(seg_ids, anchors, strict=True)):
        index_value = _ns_to_dtype(int(anchor), sample_index.ns_dtype)
        fetch_requests: dict[str, FieldFetchRequest] = {}
        for key, ranges in field_ranges.items():
            output_index_values = sample_index.output_index_values(index_value, fields[key])
            has_keyframe = is_video_field(fields[key])
            fetch_requests[key] = FieldFetchRequest(
                sample_position=i,
                segment_id=segment_id,
                index_value=index_value,
                decode_index_range=(
                    _ns_to_dtype(int(ranges[0][i]), sample_index.ns_dtype),
                    _ns_to_dtype(int(ranges[1][i]), sample_index.ns_dtype),
                ),
                output_index_values=output_index_values,
                fill_latest_at=fields[key].fill_latest_at,
                requires_contiguous_fetch=has_keyframe,
                starts_at_keyframe=has_keyframe,
            )
        targets.append(
            Target(
                segment=SegmentMetadata(segment_id=segment_id, index_start=0, index_end=0, num_samples=0),
                index_value=index_value,
                fetch_requests=fetch_requests,
            )
        )
    return targets
