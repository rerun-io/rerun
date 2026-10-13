from __future__ import annotations

import gc
import os
import sys
import time
from dataclasses import replace
from multiprocessing.reduction import ForkingPickler
from typing import TYPE_CHECKING, Any, cast

import numpy as np
import pytest
import torch
from rerun.experimental.dataloader import PooledBatchCollator, Yuv420Frame
from torch.utils.data import DataLoader, Dataset, default_collate

if TYPE_CHECKING:
    from collections.abc import Callable

# PyTorch's TreeSpec unpickler uses the deprecated LeafSpec alias internally.
pytestmark = pytest.mark.filterwarnings(
    r"ignore:^`isinstance\(treespec, LeafSpec\)` is deprecated, "
    r"use `isinstance\(treespec, TreeSpec\) and treespec\.is_leaf\(\)` instead\.$:FutureWarning"
)
requires_linux = pytest.mark.skipif(sys.platform != "linux", reason="Shared-memory FD transport requires Linux")


def sample(index: int) -> dict[str, Any]:
    return {
        "image": (torch.arange(24, dtype=torch.uint8) + index).reshape(4, 6)[:, ::2],
        "state": np.arange(3, dtype=np.float32) + index,
        "index": index,
    }


def structured_sample(index: int) -> dict[str, Any]:
    return {
        **sample(index),
        "video": Yuv420Frame(
            y=(torch.arange(35, dtype=torch.uint8) + index + 16).reshape(1, 5, 7),
            uv=(torch.arange(24, dtype=torch.uint8) + index + 112).reshape(2, 3, 4),
            color_space="bt709",
            color_range="limited",
        ),
        "nested": [(np.array([index], dtype=np.float64), index + 0.5)],
    }


class StructuredSamples(Dataset[dict[str, Any]]):
    def __len__(self) -> int:
        return 5

    def __getitem__(self, index: int) -> dict[str, Any]:
        return structured_sample(index)


def wait_until(predicate: Callable[[], bool]) -> None:
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        gc.collect()
        if predicate():
            return
        time.sleep(0.01)
    raise AssertionError("lease state did not settle")


def assert_structured_batch(actual: dict[str, Any], start: int, count: int, *, shared: bool) -> None:
    indices = range(start, start + count)
    expected = default_collate([sample(index) for index in indices])
    assert set(actual) == {*expected, "video", "nested"}
    for name, tensor in expected.items():
        torch.testing.assert_close(actual[name], tensor, rtol=0, atol=0)
        assert actual[name].is_contiguous()
        assert actual[name].is_shared() == shared
    frame = actual["video"]
    expected_frame = Yuv420Frame.stack([structured_sample(index)["video"] for index in indices])
    assert isinstance(frame, Yuv420Frame)
    assert (frame.color_space, frame.color_range) == (expected_frame.color_space, expected_frame.color_range)
    torch.testing.assert_close(frame.y, expected_frame.y)
    torch.testing.assert_close(frame.uv, expected_frame.uv)
    assert isinstance(actual["nested"], list) and len(actual["nested"]) == 1
    assert isinstance(actual["nested"][0], tuple)
    array, scalar = actual["nested"][0]
    torch.testing.assert_close(array, torch.tensor([[index] for index in indices], dtype=torch.float64))
    torch.testing.assert_close(scalar, torch.arange(start, start + count, dtype=torch.float64) + 0.5)
    assert all(tensor.is_shared() == shared for tensor in (frame.y, frame.uv, array, scalar))


def test_zero_workers_preserve_nested_samples() -> None:
    collator = PooledBatchCollator()
    actual = collator([structured_sample(0), structured_sample(1)])
    assert_structured_batch(actual, 0, 2, shared=False)
    assert collator.stats()["pool_bytes"] == 0


@requires_linux
def test_buffers_reuse_only_after_views_are_released() -> None:
    collator = PooledBatchCollator(slots_per_field=1, max_pool_bytes=64)

    def roundtrip(index: int) -> dict[str, torch.Tensor]:
        batch = collator._collate([sample(index), sample(index + 1)])
        return cast("dict[str, torch.Tensor]", ForkingPickler.loads(ForkingPickler.dumps(batch)))

    batch = roundtrip(0)
    retained = batch["image"].numpy()[:, :, ::2]
    snapshot = retained.copy()
    del batch
    wait_until(lambda: all(group["busy"] == 0 for name, group in collator.stats()["groups"].items() if name != "image"))
    before = collator.stats()
    assert before["groups"]["image"]["busy"] == 1
    changed = roundtrip(10)
    for name, expected in default_collate([sample(10), sample(11)]).items():
        torch.testing.assert_close(changed[name], expected, rtol=0, atol=0)
        assert changed[name].is_shared()
    np.testing.assert_array_equal(retained, snapshot)
    after = collator.stats()
    assert after["fallbacks"] == before["fallbacks"] + 1
    assert after["reuses"] == before["reuses"] + 2
    assert after["pool_bytes"] == 64
    assert all(group["slots"] == 1 for group in after["groups"].values())
    del retained, changed
    wait_until(lambda: all(group["busy"] == 0 for group in collator.stats()["groups"].values()))
    before = collator.stats()
    reused = roundtrip(20)
    after = collator.stats()
    assert after["reuses"] == before["reuses"] + len(reused)
    assert after["allocations"] == before["allocations"]


@requires_linux
def test_spawned_dataloader_preserves_nested_samples_across_epochs() -> None:
    loader = DataLoader(
        StructuredSamples(),
        batch_size=3,
        num_workers=2,
        multiprocessing_context="spawn",
        persistent_workers=True,
        collate_fn=PooledBatchCollator(slots_per_field=1),
        timeout=30,
    )
    try:
        for _epoch in range(2):
            batches = list(loader)
            assert len(batches) == 2
            assert_structured_batch(batches[0], 0, 3, shared=True)
            assert_structured_batch(batches[1], 3, 2, shared=True)
    finally:
        if loader._iterator is not None:
            loader._iterator._shutdown_workers()  # type: ignore[attr-defined]


def test_incompatible_samples_are_rejected_before_allocating() -> None:
    changed_shape = structured_sample(1)
    changed_shape["image"] = torch.zeros(1, dtype=torch.uint8)
    changed_metadata = structured_sample(1)
    changed_metadata["video"] = replace(changed_metadata["video"], color_range="full")
    for incompatible in (changed_shape, changed_metadata):
        collator = PooledBatchCollator()
        with pytest.raises(ValueError):
            collator._collate([structured_sample(0), incompatible])
        assert collator.stats()["allocations"] == 0


@pytest.mark.parametrize(
    ("name", "limit", "error"),
    [
        ("slots_per_field", 0, ValueError),
        ("max_pool_bytes", -1, ValueError),
        ("slots_per_field", True, TypeError),
        ("max_pool_bytes", 1.5, TypeError),
    ],
)
def test_pool_limits_require_positive_integers(name: str, limit: Any, error: type[Exception]) -> None:
    with pytest.raises(error, match="Pool limits must be"):
        PooledBatchCollator(**{name: limit})


def test_mixed_tensor_leaves_are_rejected_before_allocating() -> None:
    collator = PooledBatchCollator()
    with pytest.raises(TypeError, match="cannot mix tensors"):
        collator._collate([{"value": torch.tensor(1)}, {"value": 2}])
    assert collator.stats()["allocations"] == 0


@pytest.mark.filterwarnings("ignore:torch.quantize_per_tensor:UserWarning")
@pytest.mark.filterwarnings("ignore:The PyTorch API of nested tensors:UserWarning")
@pytest.mark.parametrize("kind", ["quantized", "nested"])
def test_unsupported_tensor_storage_is_rejected_before_allocating(kind: str) -> None:
    tensor = (
        torch.quantize_per_tensor(torch.ones(2), 0.1, 0, torch.qint8)
        if kind == "quantized"
        else torch.nested.nested_tensor([torch.ones(2), torch.ones(3)])
    )
    collator = PooledBatchCollator()
    with pytest.raises(ValueError, match="requires dense, non-quantized CPU tensors"):
        collator._collate([{"value": torch.ones(2)}, {"value": tensor}])
    assert collator.stats()["allocations"] == 0


@pytest.mark.parametrize(
    ("released", "requested_bytes", "pooled"), [(True, 8, True), (False, 8, False), (True, 12, False)]
)
@pytest.mark.skipif(sys.platform == "win32", reason="Lease acknowledgements use Unix pipe descriptors")
def test_pool_grows_only_released_slots_within_budget(
    monkeypatch: pytest.MonkeyPatch, released: bool, requested_bytes: int, pooled: bool
) -> None:
    monkeypatch.setattr(torch.UntypedStorage, "_new_using_fd_cpu", torch.UntypedStorage)
    collator = PooledBatchCollator(slots_per_field=1, max_pool_bytes=8)
    original_batch = collator._collate([{"value": torch.full((4,), 7, dtype=torch.uint8)}])
    original = original_batch.fields[0][0]
    assert collator._send is not None
    if released:
        assert original.token is not None
        os.write(collator._send.fileno(), original.token.to_bytes(8, "little"))
        collator._drain()

    replacement, is_pooled = collator._acquire("value", requested_bytes)

    assert is_pooled is pooled
    assert list(original.storage) == [7] * 4
    stats = collator.stats()
    assert stats["pool_bytes"] == (requested_bytes if pooled else 4)
    assert stats["groups"]["value"]["slots"] == 1
    assert stats["fallbacks"] == (0 if pooled else 1)
    if pooled:
        assert replacement.token is not None
        os.write(collator._send.fileno(), replacement.token.to_bytes(8, "little"))
        collator._drain()
        reused, _ = collator._acquire("value", requested_bytes)
        assert reused is replacement
        assert collator.stats()["allocations"] == 2
    elif released:
        reused, _ = collator._acquire("value", 4)
        assert reused is original


def test_failed_collation_releases_acquired_slots(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(torch.UntypedStorage, "_new_using_fd_cpu", torch.UntypedStorage)
    collator = PooledBatchCollator()
    samples = [{"first": torch.ones(2), "second": torch.ones(2)}]
    stack = torch.stack

    def fail_second_stack(tensors: list[torch.Tensor], *, out: torch.Tensor) -> torch.Tensor:
        if collator.stats()["allocations"] == 2:
            raise RuntimeError("stack failed")
        return stack(tensors, out=out)

    with monkeypatch.context() as context:
        context.setattr(torch, "stack", fail_second_stack)
        with pytest.raises(RuntimeError, match="stack failed"):
            collator._collate(samples)

    assert collator.stats()["allocations"] == 2
    assert all(group["busy"] == 0 for group in collator.stats()["groups"].values())
    assert not collator._busy_slots
    recovered = collator._collate(samples)
    assert len(recovered.fields) == 2
    assert collator.stats()["allocations"] == 2
    assert collator.stats()["reuses"] == 2
