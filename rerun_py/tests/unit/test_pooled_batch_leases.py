from __future__ import annotations

import os
import sys
from dataclasses import dataclass
from multiprocessing import Pipe
from typing import TYPE_CHECKING

import pytest
import torch
from rerun.experimental.dataloader import _pooled_batches as pooled

if TYPE_CHECKING:
    from multiprocessing.connection import Connection

pytestmark = pytest.mark.skipif(sys.platform == "win32", reason="Lease acknowledgements use Unix pipe descriptors")


@dataclass
class FdHandle:
    fd: int

    def detach(self) -> int:
        return self.fd


def release_handle(connection: Connection, token: int) -> pooled._BufferRelease:
    return pooled._BufferRelease(FdHandle(os.dup(connection.fileno())), token)


def receive_token(connection: Connection) -> int:
    assert connection.poll(3), "Released storage was not acknowledged"
    message = os.read(connection.fileno(), 8)
    assert len(message) == 8
    return int.from_bytes(message, "little")


@pytest.mark.parametrize("pipe_state", ["full", "closed"])
def test_unavailable_worker_does_not_block_other_releases(pipe_state: str) -> None:
    tracker = pooled._consumer_lease_tracker
    blocked_reader, blocked_writer = Pipe(duplex=False)
    healthy_reader, healthy_writer = Pipe(duplex=False)
    with blocked_reader, blocked_writer, healthy_reader, healthy_writer:
        filled = 0
        if pipe_state == "full":
            os.set_blocking(blocked_writer.fileno(), False)
            while True:
                try:
                    filled += os.write(blocked_writer.fileno(), bytes(8))
                except BlockingIOError:
                    break
        else:
            blocked_reader.close()

        blocked = torch.empty(1)
        healthy = torch.empty(1)
        tracker.track(blocked.untyped_storage(), release_handle(blocked_writer, 11))
        tracker.track(healthy.untyped_storage(), release_handle(healthy_writer, 12))
        del blocked, healthy

        assert receive_token(healthy_reader) == 12
        if pipe_state == "full":
            while filled:
                filled -= len(os.read(blocked_reader.fileno(), filled))
            assert receive_token(blocked_reader) == 11
