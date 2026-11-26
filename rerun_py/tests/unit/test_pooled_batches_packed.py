from __future__ import annotations

import copy
import gc
import os
import pickle
import sys
import time
from collections.abc import Mapping
from multiprocessing.reduction import ForkingPickler
from typing import TYPE_CHECKING, Any, cast

import numpy as np
import pytest
import torch
from rerun.experimental.dataloader import (
    PackedBatch,
    PooledBatchCollator,
    Yuv420Frame,
    _pooled_batches as pooled,
)
from torch.utils import _pytree as pytree
from torch.utils.data import DataLoader, Dataset
from torch.utils.data._utils.pin_memory import pin_memory

if TYPE_CHECKING:
    from collections.abc import Callable

pytestmark = pytest.mark.filterwarnings(
    r"ignore:^`isinstance\(treespec, LeafSpec\)` is deprecated, "
    r"use `isinstance\(treespec, TreeSpec\) and treespec\.is_leaf\(\)` instead\.$:FutureWarning"
)
requires_linux = pytest.mark.skipif(sys.platform != "linux", reason="Shared-memory FD transport requires Linux")
requires_unix = pytest.mark.skipif(sys.platform == "win32", reason="Worker lease pipes require Unix descriptors")


def packed_sample(index: int) -> dict[str, Any]:
    return {
        "state": (torch.arange(6, dtype=torch.float32) + index).reshape(2, 3).T,
        "array": (np.arange(12, dtype=np.float64) + index).reshape(3, 4)[:, ::2],
        "index": index,
        "score": index + 0.25,
        "flag": bool(index % 2),
        "empty": torch.empty(0, 3),
        "video": Yuv420Frame(
            y=(torch.arange(35, dtype=torch.uint8) + index).reshape(1, 5, 7),
            uv=(torch.arange(24, dtype=torch.uint8) + index + 112).reshape(2, 3, 4),
            color_space="bt709",
            color_range="limited",
        ),
        "nested": [(np.array([index], dtype=np.int16), {"value": index + 0.5})],
    }


class PackedSamples(Dataset[dict[str, Any]]):
    def __len__(self) -> int:
        return 5

    def __getitem__(self, index: int) -> dict[str, Any]:
        return packed_sample(index)


def _assert_values(actual: dict[str, Any], expected: dict[str, Any]) -> None:
    actual_leaves, actual_spec = pytree.tree_flatten(dict(actual))
    expected_leaves, expected_spec = pytree.tree_flatten(expected)
    assert actual_spec == expected_spec
    for value, reference in zip(actual_leaves, expected_leaves, strict=True):
        assert isinstance(value, torch.Tensor)
        assert value.dtype == reference.dtype and value.shape == reference.shape
        torch.testing.assert_close(value, reference, rtol=0, atol=0)


def _portable_batch(expected: dict[str, Any]) -> PackedBatch:
    leaves, spec = pytree.tree_flatten(expected)
    metadata = []
    size = 0
    for leaf in leaves:
        size = (size + 63) // 64 * 64
        metadata.append(pooled._TensorMetadata(dtype=leaf.dtype, shape=tuple(leaf.shape), offset_bytes=size))
        size += leaf.numel() * leaf.element_size()
    storage = torch.UntypedStorage(max(1, size))
    for leaf, layout in zip(leaves, metadata, strict=True):
        view = torch.empty(0, dtype=leaf.dtype).set_(  # type: ignore[call-overload]  # ty: ignore[no-matching-overload]
            storage, layout.offset_bytes // leaf.element_size(), leaf.shape
        )
        view.copy_(leaf)
    return PackedBatch(storage, tuple(metadata), spec, nbytes=size)


def _portable_roundtrip(serialized: pooled._SerializableBatch, monkeypatch: pytest.MonkeyPatch) -> PackedBatch:
    storage = serialized.fields[0][0].storage
    handles = []
    leases = []

    class Descriptor:
        def __init__(self, fd: int) -> None:
            self.fd = fd
            handles.append(fd)

        def detach(self) -> int:
            return os.dup(self.fd)

    with open(os.devnull, "rb") as descriptor, monkeypatch.context() as context:
        context.setattr(torch.UntypedStorage, "_share_fd_cpu_", lambda value: (descriptor.fileno(), value.nbytes()))
        context.setattr(torch.UntypedStorage, "_new_shared_fd_cpu", lambda _fd, _size: storage)
        context.setattr("multiprocessing.reduction.DupFd", Descriptor)
        context.setattr(pooled._consumer_lease_tracker, "track", lambda *_args: leases.append(True))
        rebuild, arguments = serialized.__reduce__()
        batch = cast("PackedBatch", rebuild(*arguments))
    assert len(handles) == 1 + int(serialized.fields[0][2])
    assert len(leases) == int(serialized.fields[0][2])
    return batch


def _wait_until(predicate: Callable[[], bool]) -> None:
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        gc.collect()
        if predicate():
            return
        time.sleep(0.01)
    raise AssertionError("The packed lease did not reach its expected state")


def _busy(collator: PooledBatchCollator) -> int:
    return sum(group["busy"] for group in collator.stats()["groups"].values())


def _disable_accelerator(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(torch.cuda, "is_available", lambda: False)
    if hasattr(torch, "accelerator"):
        monkeypatch.setattr(torch.accelerator, "is_available", lambda: False)


@pytest.mark.parametrize("packed", [False, True])
@pytest.mark.parametrize("pin_memory_requested", [False, True])
@pytest.mark.filterwarnings("ignore:.*pin_memory.*no accelerator.*:UserWarning")
def test_zero_workers_keep_normal_dictionary_fallback_and_numeric_semantics(
    monkeypatch: pytest.MonkeyPatch, packed: bool, pin_memory_requested: bool
) -> None:
    _disable_accelerator(monkeypatch)
    samples = [packed_sample(0), packed_sample(1)]
    collator = PooledBatchCollator(packed=packed)
    loader = DataLoader(
        PackedSamples(), batch_size=2, num_workers=0, pin_memory=pin_memory_requested, collate_fn=collator
    )
    batch = next(iter(loader))
    assert type(batch) is dict
    _assert_values(batch, PooledBatchCollator()(samples))
    assert batch["flag"].dtype == torch.bool and batch["score"].dtype == torch.float64
    assert batch["array"].dtype == torch.float64 and batch["empty"].shape == (2, 0, 3)
    assert collator.stats()["pool_bytes"] == 0


def test_packed_batch_supports_real_dictionary_operations() -> None:
    expected = PooledBatchCollator()([packed_sample(0), packed_sample(1)])
    batch = _portable_batch(expected)

    assert isinstance(batch, dict)
    assert isinstance(batch, Mapping)
    assert dict.__len__(batch) == len(expected)
    assert dict.__getitem__(batch, "state") is batch["state"]
    expanded = {**batch}
    assert type(expanded) is dict and expanded["state"] is batch["state"]

    metadata = {"prompt": "example task"}
    batch.update(metadata=metadata, removed=True)
    del batch["removed"]
    assert batch.setdefault("metadata", {}) is metadata
    assert "removed" not in batch and batch.get("metadata") is metadata

    for clone in (batch.copy(), copy.copy(batch)):
        assert type(clone) is dict
        assert clone["state"] is batch["state"] and clone["metadata"] is metadata
        clone["metadata_only_in_copy"] = True
        assert "metadata_only_in_copy" not in batch
    _assert_values({key: batch[key] for key in expected}, expected)


def test_packed_batch_deepcopy_clones_values_and_preserves_metadata_aliases_and_cycles() -> None:
    expected = PooledBatchCollator()([packed_sample(0), packed_sample(1)])
    batch = _portable_batch(expected)
    batch["metadata"] = {"labels": ["example task"]}
    batch["state_alias"] = batch["state"]
    batch["self"] = batch

    cloned = copy.deepcopy(batch)

    assert type(cloned) is dict
    assert cloned["self"] is cloned and cloned["state_alias"] is cloned["state"]
    _assert_values({key: cloned[key] for key in expected}, expected)
    assert cloned["state"].data_ptr() != batch["state"].data_ptr()
    assert cloned["video"].y.data_ptr() != batch["video"].y.data_ptr()
    cloned["state"].fill_(-99)
    cloned["metadata"]["labels"].append("cloned")
    assert batch["metadata"]["labels"] == ["example task"]
    _assert_values({key: batch[key] for key in expected}, expected)


def test_packed_batch_is_a_registered_pytree_with_nested_yuv_values() -> None:
    expected = PooledBatchCollator()([packed_sample(0), packed_sample(1)])
    batch = _portable_batch(expected)

    leaves, spec = pytree.tree_flatten(batch)
    references, _ = pytree.tree_flatten(expected)
    assert len(leaves) == len(references)
    assert all(isinstance(value, torch.Tensor) for value in leaves)
    for value, reference in zip(leaves, references, strict=True):
        torch.testing.assert_close(value, reference, rtol=0, atol=0)

    rebuilt = pytree.tree_unflatten(leaves, spec)
    assert type(rebuilt) is dict
    assert isinstance(rebuilt["video"], Yuv420Frame)
    _assert_values(rebuilt, expected)
    mapped = pytree.tree_map(lambda value: value.clone(), batch)
    assert type(mapped) is dict
    _assert_values(mapped, expected)
    assert mapped["state"].data_ptr() != batch["state"].data_ptr()
    assert mapped["video"].color_space == batch["video"].color_space
    assert mapped["video"].color_range == batch["video"].color_range


def test_bulk_pinning_supports_torch_without_named_tensor_attributes(monkeypatch: pytest.MonkeyPatch) -> None:
    expected = PooledBatchCollator()([packed_sample(0), packed_sample(1)])
    batch = _portable_batch(expected)
    copies = []

    def unavailable(_tensor: torch.Tensor) -> None:
        raise AttributeError("Named tensors are unavailable")

    def pin(tensor: torch.Tensor) -> torch.Tensor:
        copies.append(tensor.clone())
        return copies[-1]

    monkeypatch.setattr(torch.Tensor, "has_names", property(unavailable), raising=False)
    monkeypatch.setattr(torch.Tensor, "names", property(unavailable), raising=False)
    monkeypatch.setattr(torch.Tensor, "pin_memory", pin)

    actual = pin_memory(batch)

    assert len(copies) == 1 and copies[0].dtype == torch.uint8
    _assert_values(actual, expected)


@requires_unix
def test_mixed_dtype_nested_yuv_collation_uses_one_aligned_storage_and_one_transfer(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(torch.UntypedStorage, "_new_using_fd_cpu", torch.UntypedStorage)
    samples = [packed_sample(0), packed_sample(1)]
    expected = PooledBatchCollator()(samples)
    collator = PooledBatchCollator(packed=True)
    serialized = collator._collate(samples)
    assert len(serialized.fields) == 1
    assert collator.stats()["allocations"] == 1
    assert len(collator.stats()["groups"]) == 1
    batch = _portable_roundtrip(serialized, monkeypatch)
    assert isinstance(batch, PackedBatch)
    assert isinstance(batch, dict)
    assert isinstance(batch, Mapping)
    actual = batch
    _assert_values(actual, expected)
    leaves, _ = pytree.tree_flatten(actual)
    assert len({leaf.untyped_storage()._cdata for leaf in leaves}) == 1
    assert all((leaf.storage_offset() * leaf.element_size()) % 64 == 0 for leaf in leaves)
    with pytest.raises(RuntimeError, match="only once"):
        serialized.__reduce__()


def test_one_pin_copy_preserves_current_nested_values_metadata_and_retained_aliases(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    expected = PooledBatchCollator()([packed_sample(0), packed_sample(1)])
    batch = _portable_batch(expected)
    old_tensor = batch["state"][:, :, ::2]
    old_numpy = batch["array"].numpy()[:, :, ::2]
    tensor_snapshot = old_tensor.clone()
    numpy_snapshot = old_numpy.copy()
    batch["state"] = batch["state"][:, ::2, :]
    batch["prompt"] = ["example task"] * 2
    batch["_dropped"] = 0
    batch["preprocessing"] = {"train": True, "size": 224}
    current_state = batch["state"].clone()
    copies = []

    def pin(tensor: torch.Tensor) -> torch.Tensor:
        assert tensor.dtype == torch.uint8
        copies.append(tensor.clone())
        return copies[-1]

    monkeypatch.setattr(torch.Tensor, "pin_memory", pin)
    result = pin_memory(batch)
    assert type(result) is dict and len(copies) == 1
    assert result["prompt"] == ["example task"] * 2
    assert result["_dropped"] == 0 and result["preprocessing"] == {"train": True, "size": 224}
    assert batch.get("_dropped") == 0 and batch.get("missing", "default") == "default"
    assert dict(batch.items())["prompt"] == result["prompt"]
    torch.testing.assert_close(result["state"], current_state, rtol=0, atol=0)
    assert result["state"].stride() == batch["state"].stride()
    result["state"].fill_(-99)
    torch.testing.assert_close(old_tensor, tensor_snapshot, rtol=0, atol=0)
    np.testing.assert_array_equal(old_numpy, numpy_snapshot)
    leaves, _ = pytree.tree_flatten({key: result[key] for key in expected})
    assert all(leaf.untyped_storage()._cdata == copies[0].untyped_storage()._cdata for leaf in leaves)


def test_external_tensor_replacements_are_pinned_normally_without_losing_metadata(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    batch = _portable_batch(PooledBatchCollator()([packed_sample(0), packed_sample(1)]))
    external = torch.tensor([[7.0], [8.0]])
    batch["state"] = external
    batch["extra"] = {"tensor": torch.tensor([9], dtype=torch.int16), "label": "metadata"}
    calls = []

    def pin(tensor: torch.Tensor) -> torch.Tensor:
        calls.append(tensor)
        return tensor.clone()

    monkeypatch.setattr(torch.Tensor, "pin_memory", pin)
    result = pin_memory(batch)
    assert len(calls) == 3
    assert sum(tensor.dtype == torch.uint8 for tensor in calls) == 1
    torch.testing.assert_close(result["state"], external, rtol=0, atol=0)
    assert result["state"].data_ptr() != external.data_ptr()
    assert result["extra"]["label"] == "metadata"
    assert result["extra"]["tensor"].item() == 9


@requires_unix
def test_partial_stack_failure_releases_the_single_packed_slot(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(torch.UntypedStorage, "_new_using_fd_cpu", torch.UntypedStorage)
    collator = PooledBatchCollator(packed=True)
    stack = torch.stack
    calls = 0

    def fail_second(tensors: list[torch.Tensor], *, out: torch.Tensor) -> torch.Tensor:
        nonlocal calls
        calls += 1
        if calls == 2:
            raise RuntimeError("stack failed")
        return stack(tensors, out=out)

    with monkeypatch.context() as context:
        context.setattr(torch, "stack", fail_second)
        with pytest.raises(RuntimeError, match="stack failed"):
            collator._collate([packed_sample(0), packed_sample(1)])
    assert collator.stats()["allocations"] == 1
    assert _busy(collator) == 0 and not collator._busy_slots
    recovered = collator._collate([packed_sample(0), packed_sample(1)])
    assert len(recovered.fields) == 1
    assert collator.stats()["allocations"] == 1 and collator.stats()["reuses"] == 1


@requires_unix
def test_packed_pool_budget_falls_back_without_retaining_oversized_storage(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(torch.UntypedStorage, "_new_using_fd_cpu", torch.UntypedStorage)
    collator = PooledBatchCollator(packed=True, slots_per_field=1, max_pool_bytes=1)
    serialized = collator._collate([packed_sample(0), packed_sample(1)])
    assert len(serialized.fields) == 1
    assert collator.stats()["pool_bytes"] == 0 and collator.stats()["fallbacks"] == 1
    assert serialized.fields[0][2] is False
    _assert_values(
        _portable_roundtrip(serialized, monkeypatch),
        PooledBatchCollator()([packed_sample(0), packed_sample(1)]),
    )


@requires_linux
def test_retained_tensor_and_numpy_views_keep_the_entire_packed_lease_busy(monkeypatch: pytest.MonkeyPatch) -> None:
    collator = PooledBatchCollator(packed=True, slots_per_field=1, max_pool_bytes=1 << 20)

    def receive(index: int) -> PackedBatch:
        return cast(
            "PackedBatch", ForkingPickler.loads(ForkingPickler.dumps(collator._collate([packed_sample(index)])))
        )

    batch = receive(0)
    tensor = batch["state"][:, :, ::2]
    array = batch["array"].numpy()[:, :, ::2]
    tensor_copy, array_copy = tensor.clone(), array.copy()
    pin_calls = []

    def pin(value: torch.Tensor) -> torch.Tensor:
        pin_calls.append(value.numel())
        return value.clone()

    monkeypatch.setattr(torch.Tensor, "pin_memory", pin)
    pinned = batch.pin_memory()
    assert len(pin_calls) == 1
    del batch
    assert _busy(collator) == 1
    changed = receive(10)
    assert collator.stats()["fallbacks"] == 1
    torch.testing.assert_close(tensor, tensor_copy, rtol=0, atol=0)
    np.testing.assert_array_equal(array, array_copy)
    del changed, tensor
    gc.collect()
    assert _busy(collator) == 1
    del array
    _wait_until(lambda: _busy(collator) == 0)
    allocations = collator.stats()["allocations"]
    reused = receive(20)
    assert collator.stats()["allocations"] == allocations and collator.stats()["reuses"] == 1
    torch.testing.assert_close(reused["state"], packed_sample(20)["state"].unsqueeze(0), rtol=0, atol=0)
    torch.testing.assert_close(pinned["state"], packed_sample(0)["state"].unsqueeze(0), rtol=0, atol=0)


@requires_linux
@pytest.mark.parametrize("pin_memory_requested", [False, True])
@pytest.mark.filterwarnings("ignore:.*pin_memory.*no accelerator.*:UserWarning")
def test_spawned_workers_emit_packed_batches_across_persistent_epochs_without_accelerator(
    monkeypatch: pytest.MonkeyPatch, pin_memory_requested: bool
) -> None:
    _disable_accelerator(monkeypatch)
    loader = DataLoader(
        PackedSamples(),
        batch_size=3,
        num_workers=2,
        multiprocessing_context="spawn",
        persistent_workers=True,
        prefetch_factor=1,
        pin_memory=pin_memory_requested,
        collate_fn=PooledBatchCollator(packed=True, slots_per_field=1),
        timeout=30,
    )
    try:
        for _ in range(2):
            batches = list(loader)
            for start, batch in zip((0, 3), batches, strict=True):
                assert isinstance(batch, PackedBatch)
                assert isinstance(batch, dict)
                expected = PooledBatchCollator()([packed_sample(index) for index in range(start, min(start + 3, 5))])
                _assert_values(batch, expected)
                assert all(leaf.is_shared() for leaf in pytree.tree_flatten(batch)[0])
                assert {**batch}["state"] is batch["state"]
    finally:
        if loader._iterator is not None:
            loader._iterator._shutdown_workers()  # type: ignore[attr-defined]


def test_packed_configuration_survives_pickle() -> None:
    collator = pickle.loads(pickle.dumps(PooledBatchCollator(packed=True, slots_per_field=3, max_pool_bytes=512)))
    assert collator.packed is True and collator.slots_per_field == 3 and collator.max_pool_bytes == 512


@pytest.mark.parametrize("value", [1, None, "packed"])
def test_packed_configuration_rejects_non_boolean_flag(value: object) -> None:
    with pytest.raises(TypeError, match="packed"):
        PooledBatchCollator(packed=value)  # type: ignore[arg-type]


def test_consumer_packed_batch_cannot_be_forwarded_without_cloning() -> None:
    batch = _portable_batch(PooledBatchCollator()([packed_sample(0)]))
    with pytest.raises(RuntimeError, match="cannot be forwarded"):
        pickle.dumps(batch)


@requires_unix
def test_all_empty_leaves_roundtrip_without_inventing_elements(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(torch.UntypedStorage, "_new_using_fd_cpu", torch.UntypedStorage)
    samples = [{"tensor": torch.empty(0, 3), "array": np.empty((0, 2), dtype=np.float64)}] * 2
    serialized = PooledBatchCollator(packed=True)._collate(samples)
    assert len(serialized.fields) == 1
    actual = _portable_roundtrip(serialized, monkeypatch)
    assert actual["tensor"].shape == (2, 0, 3) and actual["array"].shape == (2, 0, 2)
    assert actual["tensor"].numel() == actual["array"].numel() == 0
    assert actual["tensor"].dtype == torch.float32 and actual["array"].dtype == torch.float64


@pytest.mark.parametrize("kind", ["conjugate", "negative", "requires_grad"])
def test_modified_tensor_view_flags_preserve_normal_pinning_values_and_autograd(
    monkeypatch: pytest.MonkeyPatch, kind: str
) -> None:
    values = torch.tensor([1 + 2j, 3 - 4j]) if kind == "conjugate" else torch.tensor([1.0, 3.0])
    batch = _portable_batch({"value": values})
    if kind == "conjugate":
        replacement = batch["value"].conj()
    elif kind == "negative":
        replacement = torch._neg_view(batch["value"])
    else:
        replacement = batch["value"].requires_grad_()
    batch["value"] = replacement
    expected = replacement.clone()
    monkeypatch.setattr(torch.Tensor, "pin_memory", lambda tensor: tensor.clone())
    actual = pin_memory(batch)["value"]
    torch.testing.assert_close(actual, expected, rtol=0, atol=0)
    assert actual.requires_grad == expected.requires_grad
    if kind == "requires_grad":
        actual.sum().backward()
        torch.testing.assert_close(replacement.grad, torch.ones_like(replacement), rtol=0, atol=0)
