from __future__ import annotations

import sys

import pytest
import torch
from rerun.experimental.dataloader import PooledBatchCollator, Yuv420Frame
from torch.utils.data import DataLoader

from .test_pooled_batches import StructuredSamples, structured_sample

# PyTorch's TreeSpec unpickler uses the deprecated LeafSpec alias internally.
pytestmark = pytest.mark.filterwarnings(
    r"ignore:^`isinstance\(treespec, LeafSpec\)` is deprecated, "
    r"use `isinstance\(treespec, TreeSpec\) and treespec\.is_leaf\(\)` instead\.$:FutureWarning"
)


@pytest.mark.skipif(sys.platform != "linux" or not torch.cuda.is_available(), reason="Requires Linux and CUDA")
def test_spawned_dataloader_pins_yuv_and_tensors_for_cuda_transfer() -> None:
    dataset = StructuredSamples()
    loader = DataLoader(
        dataset,
        batch_size=2,
        num_workers=1,
        collate_fn=PooledBatchCollator(slots_per_field=1, max_pool_bytes=1 << 20),
        multiprocessing_context="spawn",
        pin_memory=True,
        prefetch_factor=1,
        timeout=30,
    )
    batches = list(loader)
    for start, batch in zip(range(0, len(dataset), 2), batches, strict=True):
        frame = batch["video"]
        assert all(tensor.is_pinned() for tensor in (frame.y, frame.uv, batch["index"]))
        count = min(2, len(dataset) - start)
        expected = Yuv420Frame.stack([structured_sample(index)["video"] for index in range(start, start + count)])

        rgb = frame.to_rgb("cuda", non_blocking=True)
        indices = batch["index"].to("cuda", non_blocking=True)

        assert rgb.device.type == "cuda"
        torch.testing.assert_close(rgb.cpu(), expected.to_rgb("cpu"), rtol=1e-5, atol=1e-6)
        torch.testing.assert_close(indices.cpu(), torch.arange(start, start + count), rtol=0, atol=0)
