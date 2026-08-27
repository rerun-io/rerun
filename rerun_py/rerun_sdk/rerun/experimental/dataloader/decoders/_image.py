"""Decoder for encoded-image blobs."""

from __future__ import annotations

import io
from typing import TYPE_CHECKING

import torch
from PIL import Image
from torchvision.transforms.functional import pil_to_tensor  # type: ignore[import-untyped]

from rerun._tracing import with_tracing

from ._arrow import _flatten_blob
from ._base import ColumnDecoder

if TYPE_CHECKING:
    from collections.abc import Sequence

    import pyarrow as pa

    from ._base import DecodeRequest, FieldBatch


class ImageDecoder(ColumnDecoder[torch.Tensor]):
    """Decode encoded-image blobs (JPEG/PNG) to `[C, H, W]` uint8 tensors."""

    @with_tracing("ImageDecoder.decode")
    def decode(
        self,
        batch: FieldBatch,
        requests: Sequence[DecodeRequest],
    ) -> list[torch.Tensor | None]:
        return [self._decode_request(batch, request) for request in requests]

    def _decode_request(self, batch: FieldBatch, request: DecodeRequest) -> torch.Tensor | None:
        output_rows = batch.take_output_rows(request)
        if output_rows.null_count:
            return None
        if not batch.is_windowed:
            return self._decode_one(output_rows)
        frames: list[torch.Tensor] = []
        for row in range(len(output_rows)):
            frame = self._decode_one(output_rows.slice(row, 1))
            if frame is None:
                return None
            frames.append(frame)
        if any(frame.shape != frames[0].shape for frame in frames[1:]):
            return None
        return torch.stack(frames)

    @staticmethod
    def _decode_one(raw: pa.Array) -> torch.Tensor | None:
        try:
            blob_bytes = bytes(_flatten_blob(raw, 0))
            with Image.open(io.BytesIO(blob_bytes)) as image:
                return pil_to_tensor(image)  # type: ignore[no-any-return]
        except (OSError, ValueError):
            return None
