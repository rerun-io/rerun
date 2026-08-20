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


class ImageDecoder(ColumnDecoder):
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
        if not batch.is_windowed:
            return self._decode_one(output_rows)
        return torch.stack([self._decode_one(output_rows.slice(row, 1)) for row in range(len(output_rows))])

    @staticmethod
    def _decode_one(raw: pa.Array) -> torch.Tensor:
        blob_bytes = bytes(_flatten_blob(raw, 0))
        image = Image.open(io.BytesIO(blob_bytes))
        return pil_to_tensor(image)  # type: ignore[no-any-return]
