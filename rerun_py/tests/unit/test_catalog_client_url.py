"""Tests for the URL schemes accepted by `CatalogClient`."""

from __future__ import annotations

import pytest
from rerun.catalog import CatalogClient
from rerun.server import Server


def test_catalog_client_accepts_http_url(caplog: pytest.LogCaptureFixture) -> None:
    with Server() as server:
        http_url = server.url().replace("rerun+http://", "http://", 1)

        client = CatalogClient(http_url)

        assert client.url == server.url()
        assert caplog.messages == [
            f"Use `{server.url()}` instead of `{http_url}`. The `rerun+` prefix marks a Rerun data protocol URL."
        ]
        client.version_info()


def test_catalog_client_rejects_unknown_scheme() -> None:
    with pytest.raises(ValueError, match="Invalid or missing scheme"):
        CatalogClient("ftp://example.com:443")
