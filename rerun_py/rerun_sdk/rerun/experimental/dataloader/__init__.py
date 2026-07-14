"""PyTorch Datasets for training on data from the Rerun catalog."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from rerun._tracing import tracing_scope, with_tracing

from ._config import DataSource, Field
from ._sample_index import (
    FixedRateSampling,
    SampleIndex,
    SegmentMetadata,
)

if TYPE_CHECKING:
    from ._decoders import ColumnDecoder, ImageDecoder, NumericDecoder, VideoFrameDecoder
    from ._iterable_dataset import RerunIterableDataset
    from ._map_dataset import RerunMapDataset

__all__ = [
    "ColumnDecoder",
    "DataSource",
    "Field",
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

# These names require the optional `dataloader` extra (torch, av, torchvision,
# pillow); they are imported lazily (PEP 562) so the package imports without the
# extra, and decoding pulls it in only on first use.
_LAZY_SUBMODULES = {
    "ColumnDecoder": "._decoders",
    "ImageDecoder": "._decoders",
    "NumericDecoder": "._decoders",
    "VideoFrameDecoder": "._decoders",
    "RerunIterableDataset": "._iterable_dataset",
    "RerunMapDataset": "._map_dataset",
}


def __getattr__(name: str) -> Any:
    submodule = _LAZY_SUBMODULES.get(name)
    if submodule is None:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    from importlib import import_module

    return getattr(import_module(submodule, __name__), name)


def __dir__() -> list[str]:
    return sorted(__all__)
