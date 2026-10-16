"""Opt-in shared CPU buffer pooling for Linux DataLoader workers."""

from __future__ import annotations

import logging
import multiprocessing as mp
import os
import sys
import threading
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
    from collections.abc import Iterable

_FieldKey = str | tuple[str, int]

# Fixed-size pipe writes fit within PIPE_BUF, so nonblocking writes cannot split an acknowledgement.
_ACK_BYTES = 8
_LEASE_POLL_INTERVAL_SECS = 0.005


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
    tensor.set_(storage, 0, metadata.shape)  # type: ignore[call-overload]  # ty: ignore[no-matching-overload]
    return tensor


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
        buffer_fd = field.buffer_handle.detach()
        try:
            storage = torch.UntypedStorage._new_shared_fd_cpu(  # type: ignore[attr-defined]
                buffer_fd, field.buffer_size_bytes
            )
        finally:
            # The reconstructed storage owns its mapping independently of this transferred descriptor.
            os.close(buffer_fd)
        if field.release is not None:
            _consumer_lease_tracker.track(storage, field.release)
        tensors.append(_view(storage, field.tensor_metadata))
    return cast("dict[str, Any]", pytree.tree_unflatten(tensors, spec))


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
    ) -> None:
        self.spec = spec
        self.fields = fields
        self.release_connection = release_connection
        self.sent = False

    def __reduce__(self) -> Any:
        from multiprocessing.reduction import DupFd

        if self.sent:
            raise RuntimeError("A pooled batch can be serialized only once")
        self.sent = True
        fields = []
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

    """

    def __init__(self, *, slots_per_field: int = 4, max_pool_bytes: int = 4 * 1024**3) -> None:
        limits: tuple[object, object] = (slots_per_field, max_pool_bytes)
        if any(not isinstance(limit, int) or isinstance(limit, bool) for limit in limits):
            raise TypeError("Pool limits must be integers")
        if slots_per_field < 1 or max_pool_bytes < 1:
            raise ValueError("Pool limits must be positive")
        self.slots_per_field = slots_per_field
        self.max_pool_bytes = max_pool_bytes
        self._slots: dict[_FieldKey, list[_Slot]] = {}
        self._busy_slots: dict[int, _Slot] = {}
        self._recv: Connection | None = None
        self._send: Connection | None = None
        self._token = 0
        self._allocations = self._reuses = self._fallbacks = 0

    def __getstate__(self) -> dict[str, Any]:
        return {"slots_per_field": self.slots_per_field, "max_pool_bytes": self.max_pool_bytes}

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

    def __call__(self, samples: list[dict[str, Any]]) -> Any:
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
            for key, field in tensors.items():
                first = field[0]
                metadata = _TensorMetadata(dtype=first.dtype, shape=(len(samples), *first.shape))
                nbytes = len(samples) * first.numel() * first.element_size()
                slot, pooled = self._acquire(key, nbytes)
                acquired.append((slot, metadata, pooled))
                torch.stack(field, out=_view(slot.storage, metadata))
            if self._send is None:
                self._recv, self._send = Pipe(duplex=False)
            return _SerializableBatch(spec, acquired, self._send)
        except BaseException:
            for slot, _, _ in acquired:
                if slot.token is not None:
                    self._busy_slots.pop(slot.token, None)
                slot.token = None
            raise


# One tracker serves all dataloaders in the consumer process, including views retained after a loader is gone.
# Importing creates the tracker; its monitor thread starts only when the first pooled buffer is received.
_consumer_lease_tracker = _LeaseTracker()
