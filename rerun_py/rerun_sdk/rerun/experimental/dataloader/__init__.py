"""PyTorch Dataset for training on data from the Rerun catalog."""

from __future__ import annotations

from rerun._tracing import tracing_scope, with_tracing

from ._config import Column, DataSource
from ._dataset import RerunDataset
from ._decoders import ColumnDecoder, ImageDecoder, NumericDecoder, VideoFrameDecoder
from ._sample_index import (
    FixedRateSampling,
    SampleIndex,
    SegmentMetadata,
)

__all__ = [
    "Column",
    "ColumnDecoder",
    "DataSource",
    "FixedRateSampling",
    "ImageDecoder",
    "NumericDecoder",
    "RerunDataset",
    "SampleIndex",
    "SegmentMetadata",
    "VideoFrameDecoder",
    "tracing_scope",
    "with_tracing",
]
