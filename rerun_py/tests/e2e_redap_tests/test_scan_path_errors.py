"""
Test that scan-path gRPC errors surface to Python as typed messages with a trace-id.

Exercises the three gRPC endpoints reached during `ds.reader(...).collect()`:
- `GetDatasetSchema` — intercepted by the catalog client inside `reader()`
  itself, before any scan plan is built; this path was already typed on `main`.
- `QueryDataset` — called during scan-prelude (stream setup and first
  responses). Newly rewired to typed `ApiError` by PR #1666: previously
  flattened via `exec_datafusion_err!`, losing both the kind label and the
  trace-id.
- `FetchChunks` — called during data streaming in the IO loop. Typed on
  `main` by PR #1540; kept here as a regression guard.

All three are expected to propagate a typed `ApiError` to the Python boundary
with:
- a call-site label identifying the endpoint
- the `ApiErrorKind` tag (`"(NotFound)"` here — the injection synthesizes a
  tonic `NotFound` status)
- the server-assigned trace-id
"""

from __future__ import annotations

import tempfile
from pathlib import Path

import pytest
import rerun as rr
from rerun.server import Server

# In debug builds, re_server injects this fixed trace-id into every response.
DUMMY_TRACE_ID = "abba000000000000000000000000abba"


@pytest.mark.local_only
@pytest.mark.parametrize(
    ("grpc_method", "expected_label"),
    [
        # Catalog-client path inside ds.reader() — shadows the scan-path
        # GetDatasetSchema call.
        ("GetDatasetSchema", "/GetDatasetSchema failed"),
        # Scan-path call in DataframeQueryTableProvider — newly typed by #1666.
        ("QueryDataset", "query_dataset"),
        # IO-loop call in chunk_fetcher — typed on main via #1540.
        ("FetchChunks", "FetchChunks"),
    ],
)
def test_datafusion_error_surfaces_to_python_with_trace_id(grpc_method: str, expected_label: str) -> None:
    """Verify scan-path gRPC errors surface as typed messages with trace-id."""

    with tempfile.TemporaryDirectory() as tmp_dir:
        rrd_path = Path(tmp_dir) / "test.rrd"
        with rr.RecordingStream("rerun_example_test_scan_error", recording_id="test_scan_error_rec") as rec:
            rec.save(rrd_path)
            for i in range(3):
                rec.set_time("my_index", sequence=i)
                rec.log("points", rr.Points2D([[float(i), float(i)]]))
            rec.flush()

        with Server() as server:
            client = server.client()

            ds = client.create_dataset(f"scan_error_test_{grpc_method}")
            handle = ds.register(rrd_path.absolute().as_uri())
            handle.wait(timeout_secs=10)

            # Verify the normal query path works before injecting the error.
            batches = ds.reader(index="my_index").collect()
            assert len(batches) > 0, "Expected data from normal query"

            server._inject_error(grpc_method)

            # GetDatasetSchema fails during reader construction; the other two
            # fail inside collect(). Wrap both so the test doesn't care which
            # call site raises.
            try:
                ds.reader(index="my_index").collect()
                pytest.fail(f"Expected an exception from {grpc_method} failure, but query succeeded silently")
            except Exception as exc:
                msg = str(exc)
                print(f"\n[{grpc_method}] Exception type: {type(exc).__name__}")
                print(f"[{grpc_method}] Exception message: {msg}")

                assert expected_label in msg, (
                    f"{grpc_method}: error should mention call-site label {expected_label!r}, got: {msg}"
                )
                assert "(NotFound)" in msg, (
                    f"{grpc_method}: error should include ApiErrorKind label '(NotFound)', got: {msg}"
                )
                assert DUMMY_TRACE_ID in msg, (
                    f"{grpc_method}: error should include trace-id {DUMMY_TRACE_ID}, got: {msg}"
                )
            finally:
                server._clear_injected_error(grpc_method)

            ds.delete()
