"""Tests for the hosted gRPC server sink descriptor."""

from __future__ import annotations

import pytest
import rerun as rr


def test_grpc_server_sink_creation() -> None:
    try:
        rr.GrpcServerSink()
    except RuntimeError as err:
        if "not compiled with the 'server' feature" in str(err):
            pytest.skip("Rerun SDK was built without the server feature")
        raise

    sink = rr.GrpcServerSink(
        "127.0.0.1",
        9877,
        server_memory_limit="64MiB",
        newest_first=True,
        cors_allow_origin=["https://example.com"],
    )

    assert sink.uri == "rerun+http://127.0.0.1:9877/proxy"
    assert sink == sink
    assert len({sink}) == 1
    try:
        sink.port = 1  # type: ignore[attr-defined]
    except AttributeError:
        pass
    else:
        raise AssertionError("GrpcServerSink must be frozen")
