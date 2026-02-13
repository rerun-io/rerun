"""Tests for the Server class."""

from __future__ import annotations

from pathlib import Path

import pyarrow as pa
import pytest
from rerun.server import Server

RESOURCES_DIR = Path(__file__).parent.parent / "e2e_redap_tests" / "resources"
DATASET_DIR = RESOURCES_DIR / "dataset"


def test_server_starts_and_stops() -> None:
    """Test that a server can be started and stopped."""
    server = Server()
    assert server.is_running()

    url = server.url()
    assert url.startswith("rerun+http://")

    server.shutdown()
    assert not server.is_running()


def test_server_context_manager() -> None:
    """Test that the server works as a context manager."""
    with Server() as server:
        assert server.is_running()
        url = server.url()
        assert url.startswith("rerun+http://")

    assert not server.is_running()


def test_server_custom_port() -> None:
    """Test that the server can be started on a custom port."""
    with Server(port=19876) as server:
        assert server.is_running()
        assert "19876" in server.url()


def test_server_random_port() -> None:
    """Test that the server selects a random port when none is specified."""
    with Server() as server1, Server() as server2:
        # Both servers should be running on different ports
        assert server1.is_running()
        assert server2.is_running()
        assert server1.url() != server2.url()


def test_server_shutdown_twice_raises() -> None:
    """Test that shutting down a server twice raises an error."""
    server = Server()
    server.shutdown()

    with pytest.raises(ValueError, match="not running"):
        server.shutdown()


def test_server_client_after_shutdown_raises() -> None:
    """Test that getting a client after shutdown raises an error."""
    server = Server()
    server.shutdown()

    with pytest.raises(RuntimeError, match="not running"):
        server.client()


def test_server_with_dataset_prefix() -> None:
    """Test that the server can be started with a dataset prefix (directory)."""
    with Server(datasets={"test_dataset": DATASET_DIR}) as server:
        assert server.is_running()

        client = server.client()
        ds = client.get_dataset(name="test_dataset")
        assert len(ds.segment_ids()) == 20


def test_server_with_dataset_files() -> None:
    """Test that the server can be started with explicit dataset files."""
    rrd_files = list(DATASET_DIR.glob("*.rrd"))[:3]
    assert len(rrd_files) == 3

    with Server(datasets={"my_dataset": rrd_files}) as server:
        assert server.is_running()

        client = server.client()
        ds = client.get_dataset(name="my_dataset")
        assert len(ds.segment_ids()) == 3


def test_server_with_multiple_datasets() -> None:
    """Test that the server can be started with multiple datasets."""
    rrd_files = list(DATASET_DIR.glob("*.rrd"))[:2]

    with Server(
        datasets={
            "prefix_dataset": DATASET_DIR,
            "files_dataset": rrd_files,
        }
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
        Server(datasets={"bad_dataset": rrd_file})


def test_server_dataset_files_must_exist() -> None:
    """Test that dataset file paths must exist."""
    with pytest.raises(ValueError, match="must be a RRD file"):
        Server(datasets={"bad_dataset": [Path("/nonexistent/file.rrd")]})


def test_server_dataset_files_must_be_files() -> None:
    """Test that dataset file paths must be files, not directories."""
    with pytest.raises(ValueError, match="must be a RRD file"):
        Server(datasets={"bad_dataset": [DATASET_DIR]})


def test_server_failed_table_creation_does_not_leak_entry(tmp_path: Path) -> None:
    """Regression test for https://linear.app/rerun/issue/RR-3644/create-table-failure-leads-to-unlisted-existing-table."""

    with Server() as server:
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
