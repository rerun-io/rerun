from __future__ import annotations

import sys
from typing import Any

import pytest
import torch
from rerun.experimental.dataloader import PooledBatchCollator, Yuv420Frame
from torch.utils import _pytree as pytree
from torch.utils.data import DataLoader

from .test_pooled_batches import StructuredSamples, structured_sample

# PyTorch's TreeSpec unpickler uses the deprecated LeafSpec alias internally.
pytestmark = pytest.mark.filterwarnings(
    r"ignore:^`isinstance\(treespec, LeafSpec\)` is deprecated, "
    r"use `isinstance\(treespec, TreeSpec\) and treespec\.is_leaf\(\)` instead\.$:FutureWarning"
)


@pytest.mark.skipif(sys.platform != "linux" or not torch.cuda.is_available(), reason="Requires Linux and CUDA")
@pytest.mark.parametrize("packed", [False, True])
def test_spawned_dataloader_pins_yuv_and_tensors_for_cuda_transfer(
    monkeypatch: pytest.MonkeyPatch, packed: bool
) -> None:
    original_pin = torch.Tensor.pin_memory
    pin_calls = []

    def pin(tensor: torch.Tensor, *args: Any, **kwargs: Any) -> torch.Tensor:
        pin_calls.append(tensor.dtype)
        return original_pin(tensor, *args, **kwargs)

    monkeypatch.setattr(torch.Tensor, "pin_memory", pin)
    dataset = StructuredSamples()
    loader = DataLoader(
        dataset,
        batch_size=2,
        num_workers=1,
        collate_fn=PooledBatchCollator(slots_per_field=1, max_pool_bytes=1 << 20, packed=packed),
        multiprocessing_context="spawn",
        pin_memory=True,
        prefetch_factor=1,
        timeout=30,
    )
    stream = torch.cuda.Stream()
    pending = []
    with torch.cuda.stream(stream):
        for start, batch in zip(range(0, len(dataset), 2), loader, strict=True):
            assert type(batch) is dict
            frame = batch["video"]
            leaves, _ = pytree.tree_flatten(batch)
            assert all(tensor.is_pinned() for tensor in leaves)
            if packed:
                assert len({leaf.untyped_storage()._cdata for leaf in leaves}) == 1
            count = min(2, len(dataset) - start)
            expected = Yuv420Frame.stack([structured_sample(index)["video"] for index in range(start, start + count)])

            rgb = frame.to_rgb("cuda", non_blocking=True)
            indices = batch["index"].to("cuda", non_blocking=True)
            pending.append((batch, rgb, indices, expected, start, count))

    # Pinned source views must survive until their asynchronous transfers finish.
    stream.synchronize()
    tensor_leaves = len(pytree.tree_flatten(pending[0][0])[0])
    assert len(pin_calls) == len(pending) * (1 if packed else tensor_leaves)
    if packed:
        assert all(dtype == torch.uint8 for dtype in pin_calls)
    for _source, rgb, indices, expected, start, count in pending:
        assert rgb.device.type == "cuda"
        torch.testing.assert_close(rgb.cpu(), expected.to_rgb("cpu"), rtol=1e-5, atol=1e-6)
        torch.testing.assert_close(indices.cpu(), torch.arange(start, start + count), rtol=0, atol=0)
