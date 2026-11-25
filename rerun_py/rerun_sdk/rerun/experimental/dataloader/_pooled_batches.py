"""Opt-in shared CPU buffer pooling for Linux DataLoader workers."""

from __future__ import annotations

import logging
import multiprocessing as mp
import os
import sys
import threading
from copy import deepcopy
from dataclasses import dataclass
from multiprocessing import Pipe
from multiprocessing.connection import Connection
from typing import TYPE_CHECKING, Any, Protocol, cast

import numpy as np
import torch
from torch.multiprocessing.reductions import StorageWeakRef
from torch.utils import _pytree as pytree
from torch.utils.data import default_collate, get_worker_info

from ._yuv import ColorRange, ColorSpace, Yuv420Frame

if TYPE_CHECKING:
    from collections.abc import Callable, Iterable

_FieldKey = str | tuple[str, int]

# Fixed-size pipe writes fit within PIPE_BUF, so nonblocking writes cannot split an acknowledgement.
_ACK_BYTES = 8
_LEASE_POLL_INTERVAL_SECS = 0.005
_PACKED_GROUP: _FieldKey = ("", -1)
_PACKED_ALIGNMENT = 64


class _TransferableFileDescriptor(Protocol):
    """Typing interface for handles returned by `multiprocessing.reduction.DupFd`."""

    def detach(self) -> int:
        """Consume this handle once to obtain a local descriptor that the caller must close."""
        ...


def _flatten_yuv(frame: Yuv420Frame) -> tuple[list[torch.Tensor], tuple[ColorSpace, ColorRange]]:
    return [frame.y, frame.uv], (frame.color_space, frame.color_range)


def _unflatten_yuv(leaves: Iterable[torch.Tensor], metadata: tuple[ColorSpace, ColorRange]) -> Yuv420Frame:
    y, uv = leaves
    color_space, color_range = metadata
    return Yuv420Frame(y=y, uv=uv, color_space=color_space, color_range=color_range)


if Yuv420Frame not in pytree.SUPPORTED_NODES:
    pytree.register_pytree_node(Yuv420Frame, _flatten_yuv, _unflatten_yuv)


def _flatten_samples(samples: list[dict[str, Any]]) -> tuple[dict[_FieldKey, list[Any]], pytree.TreeSpec]:
    keys = tuple(samples[0])
    if any(not isinstance(key, str) for key in keys):
        raise TypeError("Sample field names must be strings")
    if any(tuple(sample) != keys for sample in samples):
        raise ValueError("Samples must have identical ordered field names")
    first, spec = pytree.tree_flatten(dict(samples[0]))
    flattened = [first]
    for sample in samples[1:]:
        leaves, sample_spec = pytree.tree_flatten(dict(sample))
        if sample_spec != spec:
            raise ValueError("Samples must have identical structure and metadata")
        flattened.append(leaves)
    groups: list[_FieldKey] = []
    for key in keys:
        _, field_spec = pytree.tree_flatten(samples[0][key])
        if field_spec.is_leaf():
            groups.append(key)
        else:
            groups.extend((key, index) for index in range(field_spec.num_leaves))
    fields = {group: list(values) for group, values in zip(groups, zip(*flattened, strict=True), strict=True)}
    return fields, spec


@dataclass(frozen=True)
class _Lease:
    storage_ref: StorageWeakRef
    release_connection: Connection
    acknowledgement: bytes


class _LeaseTracker:
    """
    Manage a background thread that detects released batch buffers and notifies their producer processes.

    The thread watches weak storage references in the consumer process and sends
    each buffer's use ID back to its producer once all local owners, including
    tensor and NumPy views, have released the storage. The producer can then
    reuse that buffer.
    """

    def __init__(self) -> None:
        self._leases: list[_Lease] = []
        self._condition = threading.Condition()
        self._monitor: threading.Thread | None = None

    def track(self, storage: torch.UntypedStorage, release: _BufferRelease) -> None:
        release_connection = Connection(release.pipe_handle.detach(), readable=False, writable=True)
        try:
            os.set_blocking(release_connection.fileno(), False)
            lease = _Lease(
                storage_ref=StorageWeakRef(storage),
                release_connection=release_connection,
                acknowledgement=release.buffer_use_id.to_bytes(_ACK_BYTES, "little"),
            )
            with self._condition:
                if self._monitor is None:
                    monitor = threading.Thread(target=self._collect_released, name="batch-lease-release", daemon=True)
                    monitor.start()
                    self._monitor = monitor
                self._leases.append(lease)
                self._condition.notify()
        except BaseException:
            release_connection.close()
            raise

    def _collect_released(self) -> None:
        while True:
            with self._condition:
                while not self._leases:
                    self._condition.wait()
                leases = self._leases
                self._leases = []

            pending = []
            for lease in leases:
                if not lease.storage_ref.expired():
                    pending.append(lease)
                    continue
                try:
                    written = os.write(lease.release_connection.fileno(), lease.acknowledgement)
                    assert written == _ACK_BYTES
                except BlockingIOError:
                    pending.append(lease)
                    continue
                except BrokenPipeError:
                    # A worker can exit while the consumer still retains its buffers.
                    pass
                except OSError:
                    logging.getLogger(__name__).exception("Failed to acknowledge a released pooled buffer")
                lease.release_connection.close()

            with self._condition:
                self._leases.extend(pending)
                if self._leases:
                    self._condition.wait(timeout=_LEASE_POLL_INTERVAL_SECS)


@dataclass(frozen=True)
class _TensorMetadata:
    dtype: torch.dtype
    shape: tuple[int, ...]
    offset_bytes: int = 0


@dataclass(frozen=True)
class _BufferRelease:
    """
    Information the consumer needs to acknowledge one use of a pooled buffer.

    The pipe is shared by a producer's pooled buffers. `buffer_use_id` identifies
    which use has ended, allowing buffers to be released out of order.
    """

    pipe_handle: _TransferableFileDescriptor
    buffer_use_id: int


@dataclass(frozen=True)
class _FieldTransfer:
    """
    Shared storage and tensor metadata for one batch leaf.

    The storage is transferred by descriptor without copying its bytes.
    `release` is present only for pooled buffers that the producer will reuse.
    """

    buffer_handle: _TransferableFileDescriptor
    buffer_size_bytes: int
    tensor_metadata: _TensorMetadata
    release: _BufferRelease | None


def _view(storage: torch.UntypedStorage, metadata: _TensorMetadata) -> torch.Tensor:
    tensor = torch.empty(0, dtype=metadata.dtype, device="cpu")
    itemsize = tensor.element_size()
    if metadata.offset_bytes < 0 or metadata.offset_bytes % itemsize:
        raise ValueError("Pooled tensor offsets must be nonnegative and dtype-aligned")
    tensor.set_(storage, metadata.offset_bytes // itemsize, metadata.shape)  # type: ignore[call-overload]  # ty: ignore[no-matching-overload]
    return tensor


def _receive_storage(field: _FieldTransfer) -> torch.UntypedStorage:
    buffer_fd = field.buffer_handle.detach()
    try:
        storage = torch.UntypedStorage._new_shared_fd_cpu(  # type: ignore[attr-defined]
            buffer_fd, field.buffer_size_bytes
        )
    finally:
        os.close(buffer_fd)
    if field.release is not None:
        _consumer_lease_tracker.track(storage, field.release)
    return cast("torch.UntypedStorage", storage)


def _rebuild_batch(spec: pytree.TreeSpec, fields: list[_FieldTransfer]) -> dict[str, Any]:
    """
    Reconstruct a batch in the consumer process when unpickling a worker's serialized batch.

    Receive the transferred file descriptors and map their shared buffers into
    tensor views without copying the data. Track when the consumer releases its
    views so the producer can be notified that its pooled buffers are safe to reuse.
    Reassemble the tensors into the nested sample structure described by `spec`.
    """
    tensors = []
    for field in fields:
        tensors.append(_view(_receive_storage(field), field.tensor_metadata))
    return cast("dict[str, Any]", pytree.tree_unflatten(tensors, spec))


class PackedBatch(dict[str, Any]):
    """
    A dictionary whose tensor leaves share one leased CPU allocation.

    Created by [`PooledBatchCollator`][rerun.experimental.dataloader.PooledBatchCollator]
    with `packed=True`.
    When accelerator pinning is enabled, a `DataLoader` with `pin_memory=True`
    calls [`pin_memory`][rerun.experimental.dataloader.PackedBatch.pin_memory]
    and delivers an ordinary dictionary with pinned tensor views.
    If pinning is disabled or no supported accelerator is available, the loader
    delivers this dictionary subclass even when `pin_memory=True` was requested.
    With `num_workers=0`, the collator instead returns an ordinary dictionary.
    Dictionary operations and PyTorch pytree transformations work without
    an explicit unpacking step, including when pinning is disabled.

    Retaining any tensor or NumPy view retains the entire shared buffer.
    Clone tensors before forwarding them to another process.
    """

    def __init__(
        self,
        storage: torch.UntypedStorage,
        metadata: tuple[_TensorMetadata, ...],
        spec: pytree.TreeSpec,
        *,
        nbytes: int,
    ) -> None:
        self._storage = storage
        self._nbytes = nbytes
        super().__init__(pytree.tree_unflatten([_view(storage, item) for item in metadata], spec))

    def __copy__(self) -> dict[str, Any]:
        """Return a shallow dictionary whose tensor views retain their shared-buffer lease."""
        return dict(self)

    def __deepcopy__(self, memo: dict[int, Any]) -> dict[str, Any]:
        """Copy values into an ordinary dictionary without retaining packed-storage metadata."""
        result: dict[str, Any] = {}
        memo[id(self)] = result
        for key, value in self.items():
            result[deepcopy(key, memo)] = deepcopy(value, memo)
        return result

    def pin_memory(self) -> dict[str, Any]:
        """
        Copy the packed storage once and reconstruct ordinary pinned tensor views.

        Existing CPU aliases are left intact and retain their lease.
        Application metadata and changes to the batch are preserved.
        Tensors added or replaced after collation are pinned separately.
        """
        source = _view(self._storage, _TensorMetadata(torch.uint8, (self._nbytes,)))
        pinned = source.pin_memory()
        target = pinned.untyped_storage()

        def pin(value: Any) -> Any:  # noqa: ANN401
            if not isinstance(value, torch.Tensor):
                return value
            if (
                value.device.type != "cpu"
                or type(value) is not torch.Tensor
                or value.layout != torch.strided
                or value.is_nested
                or value.requires_grad
                or value.is_conj()
                or value.is_neg()
                or any(name is not None for name in getattr(value, "names", ()))
                or value.untyped_storage()._cdata != self._storage._cdata
            ):
                return value.pin_memory()
            end = value.storage_offset()
            if value.numel():
                end += 1 + sum((size - 1) * stride for size, stride in zip(value.shape, value.stride(), strict=True))
            if end * value.element_size() > target.nbytes():
                raise ValueError("A packed tensor view extends beyond the collated buffer")
            result = torch.empty(0, dtype=value.dtype, device="cpu")
            result.set_(target, value.storage_offset(), value.shape, value.stride())  # type: ignore[call-overload]
            return result

        return cast("dict[str, Any]", pytree.tree_map(pin, dict(self)))

    def __reduce__(self) -> Any:  # noqa: ANN401
        raise RuntimeError("Packed batches cannot be forwarded; clone their tensors into a dictionary first")


def _flatten_packed_batch(batch: PackedBatch) -> tuple[list[Any], list[str]]:
    return list(batch.values()), list(batch)


def _unflatten_packed_batch(values: Iterable[Any], keys: list[str]) -> dict[str, Any]:
    return dict(zip(keys, values, strict=True))


def _flatten_packed_batch_with_keys(batch: PackedBatch) -> tuple[list[tuple[pytree.KeyEntry, Any]], list[str]]:
    return [(cast("pytree.KeyEntry", pytree.MappingKey(key)), value) for key, value in batch.items()], list(batch)


if PackedBatch not in pytree.SUPPORTED_NODES:
    pytree.register_pytree_node(
        PackedBatch,
        _flatten_packed_batch,
        _unflatten_packed_batch,
        flatten_with_keys_fn=_flatten_packed_batch_with_keys,
    )


def _rebuild_packed_batch(
    spec: pytree.TreeSpec, field: _FieldTransfer, metadata: tuple[_TensorMetadata, ...]
) -> PackedBatch:
    return PackedBatch(_receive_storage(field), metadata, spec, nbytes=field.tensor_metadata.shape[0])


@dataclass
class _Slot:
    """A shared-memory buffer with a token tracking when the producer can reuse it."""

    storage: torch.UntypedStorage
    token: int | None = None


class _SerializableBatch:
    """Reconstruct consumer tensors with storage-release tracking when deserialized."""

    def __init__(
        self,
        spec: pytree.TreeSpec,
        fields: list[tuple[_Slot, _TensorMetadata, bool]],
        release_connection: Connection,
        *,
        packed_metadata: tuple[_TensorMetadata, ...] | None = None,
    ) -> None:
        self.spec = spec
        self.fields = fields
        self.release_connection = release_connection
        self.sent = False
        self.packed_metadata = packed_metadata

    def __reduce__(
        self,
    ) -> tuple[Callable[..., Any], tuple[Any, ...]]:
        from multiprocessing.reduction import DupFd

        if self.sent:
            raise RuntimeError("A pooled batch can be serialized only once")
        self.sent = True
        fields: list[_FieldTransfer] = []
        for slot, metadata, pooled in self.fields:
            buffer_fd, buffer_size_bytes = slot.storage._share_fd_cpu_()
            release = None
            if pooled:
                assert slot.token is not None
                release = _BufferRelease(DupFd(self.release_connection.fileno()), slot.token)
            fields.append(
                _FieldTransfer(
                    buffer_handle=DupFd(buffer_fd),
                    buffer_size_bytes=buffer_size_bytes,
                    tensor_metadata=metadata,
                    release=release,
                )
            )
        if self.packed_metadata is not None:
            return _rebuild_packed_batch, (self.spec, fields[0], self.packed_metadata)
        return _rebuild_batch, (self.spec, fields)


class PooledBatchCollator:
    """
    Collate CPU sample dictionaries into lazily allocated reusable shared buffers.

    Pass this collator as a PyTorch `DataLoader`'s `collate_fn` to pool worker
    output buffers. Pooling requires Linux, `multiprocessing_context="spawn"`,
    and PyTorch's `file_descriptor` sharing strategy. It uses private PyTorch
    storage APIs. With `num_workers=0`, each leaf uses `default_collate`
    without pooling, including on other platforms.

    A slot is reusable only after the receiving process releases its storage,
    including tensor and NumPy views. If every cached slot is still retained,
    a fresh allocation keeps loading moving without overwriting live outputs.
    Pooled tensors must be cloned before passing them to another process.
    Keep source tensors alive until asynchronous device transfers complete.

    Supports dense, non-quantized CPU tensors without gradients, NumPy arrays, numbers, and
    [`Yuv420Frame`][rerun.experimental.dataloader.Yuv420Frame] inside nested dictionaries, lists, and tuples.
    Samples in a batch must have matching ordered fields, structure, shapes,
    dtypes, and metadata; missing values are not supported.

    Parameters
    ----------
    slots_per_field:
        Maximum number of cached shared buffers per tensor leaf, per worker.
    max_pool_bytes:
        Maximum cached storage in bytes per worker, allocated on demand.
        This does not bound memory retained by consumers or fallback allocations
        when all cached buffers are busy. Shared memory must accommodate both
        cached and in-flight batches.
    packed:
        Store all tensor leaves in one aligned shared buffer instead of one buffer
        per leaf. `slots_per_field` then limits cached whole-batch buffers.
        When `DataLoader(pin_memory=True)` actually enables accelerator pinning,
        one parent pinning copy produces an ordinary dictionary.
        Otherwise the consumer receives the dictionary subclass
        [`PackedBatch`][rerun.experimental.dataloader.PackedBatch], including on
        CPU-only hosts where PyTorch disables the requested pinning.
        Both results support dictionary operations and PyTorch pytree transformations
        without changing the training loop.
        Retaining one leaf retains the whole packed allocation.
        The zero-worker fallback is unchanged.

    Examples
    --------
    ```python
    loader = torch.utils.data.DataLoader(
        dataset,
        batch_size=32,
        num_workers=4,
        multiprocessing_context="spawn",
        pin_memory=True,
        collate_fn=PooledBatchCollator(packed=True),
    )
    ```

    """

    def __init__(self, *, slots_per_field: int = 4, max_pool_bytes: int = 4 * 1024**3, packed: bool = False) -> None:
        limits: tuple[object, object] = (slots_per_field, max_pool_bytes)
        if any(not isinstance(limit, int) or isinstance(limit, bool) for limit in limits):
            raise TypeError("Pool limits must be integers")
        if slots_per_field < 1 or max_pool_bytes < 1:
            raise ValueError("Pool limits must be positive")
        if not isinstance(packed, bool):
            raise TypeError("packed must be a boolean")
        self.slots_per_field = slots_per_field
        self.max_pool_bytes = max_pool_bytes
        self.packed = packed
        self._slots: dict[_FieldKey, list[_Slot]] = {}
        self._busy_slots: dict[int, _Slot] = {}
        self._recv: Connection | None = None
        self._send: Connection | None = None
        self._token = 0
        self._allocations = self._reuses = self._fallbacks = 0

    def __getstate__(self) -> dict[str, Any]:
        return {"slots_per_field": self.slots_per_field, "max_pool_bytes": self.max_pool_bytes, "packed": self.packed}

    def __setstate__(self, state: dict[str, Any]) -> None:
        PooledBatchCollator.__init__(self, **state)

    def _drain(self) -> None:
        if self._recv is None:
            return
        while self._recv.poll():
            acknowledgement = os.read(self._recv.fileno(), _ACK_BYTES)
            if len(acknowledgement) != _ACK_BYTES:
                raise RuntimeError("Incomplete pooled buffer acknowledgement")
            token = int.from_bytes(acknowledgement, "little")
            slot = self._busy_slots.pop(token, None)
            if slot is not None:
                slot.token = None

    def stats(self) -> dict[str, Any]:
        """Return allocation, reuse, fallback, and buffer counts for this process's pool."""
        self._drain()
        return {
            "allocations": self._allocations,
            "reuses": self._reuses,
            "fallbacks": self._fallbacks,
            "pool_bytes": sum(slot.storage.nbytes() for slots in self._slots.values() for slot in slots),
            "groups": {
                group: {"slots": len(slots), "busy": sum(s.token is not None for s in slots)}
                for group, slots in self._slots.items()
            },
        }

    def _acquire(self, group: _FieldKey, nbytes: int) -> tuple[_Slot, bool]:
        slots = self._slots.setdefault(group, [])
        free = [slot for slot in slots if slot.token is None and slot.storage.nbytes() >= nbytes]
        if free:
            slot = min(free, key=lambda s: s.storage.nbytes())
            self._reuses += 1
            pooled = True
        else:
            current = sum(s.storage.nbytes() for group_slots in self._slots.values() for s in group_slots)
            replacement = max(
                (slot for slot in slots if slot.token is None),
                key=lambda slot: slot.storage.nbytes(),
                default=None,
            )
            slot = _Slot(torch.UntypedStorage._new_using_fd_cpu(max(1, nbytes)))
            self._allocations += 1
            retained_bytes = current - (replacement.storage.nbytes() if replacement is not None else 0)
            pooled = (
                replacement is not None or len(slots) < self.slots_per_field
            ) and retained_bytes + slot.storage.nbytes() <= self.max_pool_bytes
            if pooled:
                if replacement is None:
                    slots.append(slot)
                else:
                    slots[slots.index(replacement)] = slot
            else:
                self._fallbacks += 1
        self._token += 1
        slot.token = self._token
        if pooled:
            self._busy_slots[slot.token] = slot
        return slot, pooled

    # The batch mirrors the samples' pytree, like `default_collate`, which is also typed `Any`.
    def __call__(self, samples: list[dict[str, Any]]) -> Any:  # noqa: ANN401
        """Collate samples, pooling shared storage only inside supported DataLoader workers."""
        if not samples or not isinstance(samples[0], dict):
            return default_collate(samples)
        if get_worker_info() is None:
            fields, spec = _flatten_samples(samples)
            return pytree.tree_unflatten([default_collate(values) for values in fields.values()], spec)
        if sys.platform != "linux" or mp.get_start_method() != "spawn":
            raise RuntimeError(
                "PooledBatchCollator requires Linux DataLoader workers with multiprocessing_context='spawn'"
            )
        if torch.multiprocessing.get_sharing_strategy() != "file_descriptor":
            raise RuntimeError("PooledBatchCollator requires PyTorch's file_descriptor sharing strategy")
        return self._collate(samples)

    def _collate(self, samples: list[dict[str, Any]]) -> _SerializableBatch:
        fields, spec = _flatten_samples(samples)
        self._drain()
        tensors: dict[_FieldKey, list[torch.Tensor]] = {}
        for key, values in fields.items():
            if isinstance(values[0], torch.Tensor):
                field = values
            elif isinstance(values[0], (np.ndarray, np.generic)):
                field = [torch.as_tensor(value, device="cpu") for value in values]
            else:
                # Default collation defines Python scalar types independently of
                # torch.as_tensor defaults (in particular, float -> float64).
                with torch.device("cpu"):
                    stacked = default_collate(values)
                if not isinstance(stacked, torch.Tensor):
                    raise TypeError("PooledBatchCollator requires tensor, NumPy array, or numeric pytree leaves")
                field = list(stacked.unbind(0))
            first = field[0]
            if any(not isinstance(value, torch.Tensor) for value in field):
                raise TypeError("A tensor field cannot mix tensors with other leaf types")
            if any(
                value.is_quantized
                or value.is_nested
                or value.requires_grad
                or value.device.type != "cpu"
                or value.layout != torch.strided
                for value in field
            ):
                raise ValueError("PooledBatchCollator requires dense, non-quantized CPU tensors without gradients")
            if any(value.dtype != first.dtype or value.shape != first.shape for value in field):
                raise ValueError("Each field must have a uniform shape and dtype without gradients")
            tensors[key] = field
        acquired: list[tuple[_Slot, _TensorMetadata, bool]] = []
        try:
            packed_metadata = None
            if self.packed and tensors:
                layouts = []
                nbytes = 0
                for field in tensors.values():
                    first = field[0]
                    alignment = max(_PACKED_ALIGNMENT, first.element_size())
                    offset = (nbytes + alignment - 1) // alignment * alignment
                    layouts.append(_TensorMetadata(first.dtype, (len(samples), *first.shape), offset))
                    nbytes = offset + len(samples) * first.numel() * first.element_size()
                byte_metadata = _TensorMetadata(torch.uint8, (nbytes,))
                slot, pooled = self._acquire(_PACKED_GROUP, nbytes)
                acquired.append((slot, byte_metadata, pooled))
                packed_metadata = tuple(layouts)
                cursor = 0
                buffer = _view(slot.storage, byte_metadata)
                for field, layout in zip(tensors.values(), layouts, strict=True):
                    buffer[cursor : layout.offset_bytes].zero_()
                    torch.stack(field, out=_view(slot.storage, layout))
                    cursor = layout.offset_bytes + len(samples) * field[0].numel() * field[0].element_size()
            else:
                for key, field in tensors.items():
                    first = field[0]
                    metadata = _TensorMetadata(dtype=first.dtype, shape=(len(samples), *first.shape))
                    nbytes = len(samples) * first.numel() * first.element_size()
                    slot, pooled = self._acquire(key, nbytes)
                    acquired.append((slot, metadata, pooled))
                    torch.stack(field, out=_view(slot.storage, metadata))
            if self._send is None:
                self._recv, self._send = Pipe(duplex=False)
            return _SerializableBatch(spec, acquired, self._send, packed_metadata=packed_metadata)
        except BaseException:
            for slot, _, _ in acquired:
                if slot.token is not None:
                    self._busy_slots.pop(slot.token, None)
                slot.token = None
            raise


# One tracker serves all dataloaders in the consumer process, including views retained after a loader is gone.
# Importing creates the tracker; its monitor thread starts only when the first pooled buffer is received.
_consumer_lease_tracker = _LeaseTracker()
