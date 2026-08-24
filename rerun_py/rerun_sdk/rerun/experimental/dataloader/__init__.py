"""PyTorch Datasets for training on data from the Rerun catalog."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

from rerun._tracing import tracing_scope, with_tracing

from ._config import DataSource, Field
from ._sample_index import (
    FixedRateSampling,
    IndexValue,
    SampleIndex,
    SegmentMetadata,
)
from ._shuffle import BlockShuffle, NoShuffle, SampleShuffle, ShuffleStrategy

if TYPE_CHECKING:
    from ._iterable_dataset import RerunIterableDataset
    from ._map_dataset import RerunMapDataset
    from ._yuv import Yuv420Collator, Yuv420Frame
    from .decoders import (
        ColumnDecoder,
        DecodeRequest,
        FieldBatch,
        ImageDecoder,
        NumericDecoder,
        VideoFrameDecoder,
    )
    from .manifest._manifest import Manifest

__all__ = [
    "BlockShuffle",
    "ColumnDecoder",
    "DataSource",
    "DecodeRequest",
    "Field",
    "FieldBatch",
    "FixedRateSampling",
    "ImageDecoder",
    "IndexValue",
    "Manifest",
    "NoShuffle",
    "NumericDecoder",
    "RerunIterableDataset",
    "RerunMapDataset",
    "SampleIndex",
    "SampleShuffle",
    "SegmentMetadata",
    "ShuffleStrategy",
    "VideoFrameDecoder",
    "Yuv420Collator",
    "Yuv420Frame",
    "tracing_scope",
    "with_tracing",
]

# These names require the optional `dataloader` extra (torch, av, torchvision,
# pillow); they are imported lazily (PEP 562) so the package imports without the
# extra, and decoding pulls it in only on first use.
_LAZY_SUBMODULES = {
    "ColumnDecoder": ".decoders",
    "DecodeRequest": ".decoders",
    "FieldBatch": ".decoders",
    "ImageDecoder": ".decoders",
    "NumericDecoder": ".decoders",
    "VideoFrameDecoder": ".decoders",
    "Yuv420Collator": "._yuv",
    "Yuv420Frame": "._yuv",
    "RerunIterableDataset": "._iterable_dataset",
    "RerunMapDataset": "._map_dataset",
    "Manifest": ".manifest._manifest",
}


def __getattr__(name: str) -> Any:
    submodule = _LAZY_SUBMODULES.get(name)
    if submodule is None:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    from importlib import import_module

    return getattr(import_module(submodule, __name__), name)


def __dir__() -> list[str]:
    return sorted(__all__)
