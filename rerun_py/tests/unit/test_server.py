"""Tests for the Server class."""

from __future__ import annotations

import socket
from pathlib import Path

import pyarrow as pa
import pytest
from rerun.server import Server

RESOURCES_DIR = Path(__file__).parent.parent / "e2e_redap_tests" / "resources"
DATASET_DIR = RESOURCES_DIR / "dataset"


def _find_free_port() -> int:
    """Bind to port 0, read what the OS picked, then release it."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        port: int = s.getsockname()[1]
        return port


def test_server_starts_and_stops() -> None:
    """Test that a server can be started and stopped."""
    server = Server(host="127.0.0.1")
    assert server.is_running()

    url = server.url()
    assert url.startswith("rerun+http://")

    server.shutdown()
    assert not server.is_running()


def test_server_context_manager() -> None:
    """Test that the server works as a context manager."""
    with Server(host="127.0.0.1") as server:
        assert server.is_running()
        url = server.url()
        assert url.startswith("rerun+http://")

    assert not server.is_running()


def test_server_custom_port() -> None:
    """Test that the server can be started on a custom port."""
    port = _find_free_port()
    with Server(host="127.0.0.1", port=port) as server:
        assert server.is_running()
        assert server.url().endswith(f":{port}")


def test_server_random_port() -> None:
    """Test that the server selects a random port when none is specified."""
    with Server(host="127.0.0.1") as server1, Server(host="127.0.0.1") as server2:
        # Both servers should be running on different ports
        assert server1.is_running()
        assert server2.is_running()
        assert server1.url() != server2.url()


def test_server_ephemeral_port() -> None:
    """Regression test: `port=0` must report the OS-assigned port, not `:0`."""
    with Server(host="127.0.0.1", port=0) as server:
        url = server.url()
        assert not url.endswith(":0")

        port = int(url.rsplit(":", 1)[1])
        assert 0 < port

        # The reported URL must actually be connectable:
        server.client().version_info()


def test_server_shutdown_twice_raises() -> None:
    """Test that shutting down a server twice raises an error."""
    server = Server(host="127.0.0.1")
    server.shutdown()

    with pytest.raises(ValueError, match="not running"):
        server.shutdown()


def test_server_client_after_shutdown_raises() -> None:
    """Test that getting a client after shutdown raises an error."""
    server = Server(host="127.0.0.1")
    server.shutdown()

    with pytest.raises(RuntimeError, match="not running"):
        server.client()


def test_server_with_dataset_prefix() -> None:
    """Test that the server can be started with a dataset prefix (directory)."""
    with Server(host="127.0.0.1", datasets={"test_dataset": DATASET_DIR}) as server:
        assert server.is_running()

        client = server.client()
        ds = client.get_dataset(name="test_dataset")
        assert len(ds.segment_ids()) == 20


def test_server_with_dataset_files() -> None:
    """Test that the server can be started with explicit dataset files."""
    rrd_files = list(DATASET_DIR.glob("*.rrd"))[:3]
    assert len(rrd_files) == 3

    with Server(host="127.0.0.1", datasets={"my_dataset": rrd_files}) as server:
        assert server.is_running()

        client = server.client()
        ds = client.get_dataset(name="my_dataset")
        assert len(ds.segment_ids()) == 3


def test_server_with_multiple_datasets() -> None:
    """Test that the server can be started with multiple datasets."""
    rrd_files = list(DATASET_DIR.glob("*.rrd"))[:2]

    with Server(
        host="127.0.0.1",
        datasets={
            "prefix_dataset": DATASET_DIR,
            "files_dataset": rrd_files,
        },
    ) as server:
        assert server.is_running()

        client = server.client()

        prefix_ds = client.get_dataset(name="prefix_dataset")
        files_ds = client.get_dataset(name="files_dataset")

        assert len(prefix_ds.segment_ids()) == 20
        assert len(files_ds.segment_ids()) == 2


def test_server_dataset_prefix_must_be_directory() -> None:
    """Test that dataset prefix paths must be directories."""
    rrd_file = next(DATASET_DIR.glob("*.rrd"))

    with pytest.raises(ValueError, match="must be a directory"):
        Server(host="127.0.0.1", datasets={"bad_dataset": rrd_file})


def test_server_dataset_files_must_exist() -> None:
    """Test that dataset file paths must exist."""
    with pytest.raises(ValueError, match="must be a RRD file"):
        Server(host="127.0.0.1", datasets={"bad_dataset": [Path("/nonexistent/file.rrd")]})


def test_server_dataset_files_must_be_files() -> None:
    """Test that dataset file paths must be files, not directories."""
    with pytest.raises(ValueError, match="must be a RRD file"):
        Server(host="127.0.0.1", datasets={"bad_dataset": [DATASET_DIR]})


def test_server_version_info() -> None:
    """Test that version_info() returns valid info for a local OSS server."""
    with Server(host="127.0.0.1") as server:
        client = server.client()
        info = client.version_info()
        assert isinstance(info.version, str)
        assert info.cloud_provider is None
        assert info.cloud_region is None


def test_server_failed_table_creation_does_not_leak_entry(tmp_path: Path) -> None:
    """Regression test for https://linear.app/rerun/issue/RR-3644/create-table-failure-leads-to-unlisted-existing-table."""

    with Server(host="127.0.0.1") as server:
        client = server.client()

        schema = pa.schema([])

        try:
            # This must fail because the URI is unsupported
            t = client.create_table("test", schema, url="surprise://bucket/does/not/exist")
        except Exception:
            pass

        assert "t" not in locals()

        # We should be free to
        t = client.create_table("test", schema, url=tmp_path.as_uri())
