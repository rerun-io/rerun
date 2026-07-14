"""Integration tests for the headless viewer spawned via the Python SDK."""

from __future__ import annotations

import platform
import socket
import sys
import time
from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.experimental import ViewerClient

if TYPE_CHECKING:
    from pathlib import Path

# The wheel-test CI installs a software rasterizer only on linux-x64 (see
# `.github/workflows/rerun_reusable_test_wheels.yml`). On linux-arm64 the
# manylinux container has no Vulkan adapter, so the headless viewer panics on
# startup with "No graphics adapter was found".
pytestmark = pytest.mark.skipif(
    sys.platform == "linux" and platform.machine() == "aarch64",
    reason="no software rasterizer on linux-arm64 wheel-test runner",
)


def _find_free_port() -> int:
    """Bind to port 0, read what the OS picked, then release it."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        port: int = s.getsockname()[1]
        return port


def _wait_for_file(path: Path, timeout: float) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.exists() and path.stat().st_size > 0:
            return
        time.sleep(0.1)
    raise TimeoutError(f"screenshot was never written to {path}")


def _wait_for_port(port: int, timeout: float) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(0.2)
            try:
                s.connect(("127.0.0.1", port))
                return
            except OSError:
                time.sleep(0.1)
    raise TimeoutError(f"viewer never started listening on port {port}")


def _wait_for_port_closed(port: int, timeout: float) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(0.2)
            try:
                s.connect(("127.0.0.1", port))
            except OSError:
                return
        time.sleep(0.1)
    raise TimeoutError(f"viewer still listening on port {port} after teardown")


@pytest.mark.skip(reason="RR-5124: linux wheel CI segfaults in llvmpipe/Mesa after the RunsOn AMI rollout")
def test_save_screenshot(tmp_path: Path) -> None:
    """Log into a spawned headless viewer, then screenshot it to disk."""
    port = _find_free_port()

    with ViewerClient.spawn(headless=True, port=port, hide_welcome_screen=True) as viewer:
        rec = rr.RecordingStream("rerun_example_headless_test")
        rec.connect_grpc(url=viewer.url)
        rec.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1], [2, 0, 1]], colors=[255, 0, 0]))
        rec.flush()

        out = tmp_path / "screenshot.png"
        viewer.save_screenshot(str(out))
        _wait_for_file(out, timeout=5.0)

        # PNG magic number: 89 50 4E 47 0D 0A 1A 0A
        with out.open("rb") as f:
            assert f.read(8) == b"\x89PNG\r\n\x1a\n"


def test_viewer_dies_on_client_close() -> None:
    """Closing the ViewerClient should kill the viewer it spawned."""
    port = _find_free_port()
    viewer = ViewerClient.spawn(headless=True, port=port, hide_welcome_screen=True)

    _wait_for_port(port, timeout=30.0)
    viewer.close()
    # SIGTERM lands; the viewer should release the port within a few seconds.
    _wait_for_port_closed(port, timeout=15.0)
