"""PyTorch Datasets for training on data from the Rerun catalog."""

from __future__ import annotations

from rerun._tracing import tracing_scope, with_tracing

from ._config import Column, DataSource
from ._decoders import ColumnDecoder, ImageDecoder, NumericDecoder, VideoFrameDecoder
from ._iterable_dataset import RerunIterableDataset
from ._map_dataset import RerunMapDataset
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
    "RerunIterableDataset",
    "RerunMapDataset",
    "SampleIndex",
    "SegmentMetadata",
    "VideoFrameDecoder",
    "tracing_scope",
    "with_tracing",
]
