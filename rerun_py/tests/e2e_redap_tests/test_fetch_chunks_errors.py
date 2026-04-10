"""Test that FetchChunks gRPC errors surface to Python with useful error messages."""

from __future__ import annotations

import tempfile
from pathlib import Path

import pytest
import rerun as rr
from rerun.server import Server

# In debug builds, re_server injects this fixed trace-id into every response.
DUMMY_TRACE_ID = "abba000000000000000000000000abba"


@pytest.mark.local_only
def test_fetch_chunks_error_surfaces_to_python() -> None:
    """
    Verify that a FetchChunks gRPC error propagates all the way to Python.

    This exercises the fix for RR-4337: previously, the IO task error was silently
    dropped and the Python SDK would receive partial/empty data with no exception.

    The test uses a server-side test hook that makes FetchChunks return a NotFound
    error, then verifies that the error surfaces when collecting query results,
    including a trace-id for support correlation.
    """

    with tempfile.TemporaryDirectory() as tmp_dir:
        # Create a small recording with temporal data
        rrd_path = Path(tmp_dir) / "test.rrd"
        with rr.RecordingStream("rerun_example_test_fetch_error", recording_id="test_fetch_error_rec") as rec:
            rec.save(rrd_path)
            for i in range(3):
                rec.set_time("my_index", sequence=i)
                rec.log("points", rr.Points2D([[float(i), float(i)]]))
            rec.flush()

        with Server() as server:
            client = server.client()

            # Create and populate a test dataset
            ds = client.create_dataset("fetch_error_test")
            handle = ds.register(rrd_path.absolute().as_uri())
            handle.wait(timeout_secs=10)

            # Verify normal query works first
            batches = ds.reader(index="my_index").collect()
            assert len(batches) > 0, "Expected data from normal query"

            # Now inject the error: FetchChunks will return NotFound error
            server._inject_error("FetchChunks")

            try:
                ds.reader(index="my_index").collect()
                pytest.fail("Expected an exception from FetchChunks failure, but query succeeded silently")
            except Exception as exc:
                error_message = str(exc)
                print(f"\nException type: {type(exc).__name__}")
                print(f"Exception message: {error_message}")

                assert "FetchChunks" in error_message, f"Error should mention FetchChunks, got: {error_message}"

                # re_server injects a dummy trace-id into every response.
                # Verify it's included in the error message.
                # This is a test that trace-id:s make it all the way from the server to the user.
                assert DUMMY_TRACE_ID in error_message, (
                    f"Error should include trace-id {DUMMY_TRACE_ID}, got: {error_message}"
                )
            finally:
                server._clear_injected_error("FetchChunks")

            # Clean up
            ds.delete()
