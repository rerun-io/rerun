from __future__ import annotations

from typing import TYPE_CHECKING

from rerun import server as _server

from .catalog import CatalogClient

if TYPE_CHECKING:
    from os import PathLike
    from types import TracebackType


class Server:
    host = _server.Server.host
    is_running = _server.Server.is_running
    shutdown = _server.Server.shutdown

    def __init__(
        self,
        *,
        host: str = "::",
        port: int | None = None,
        datasets: dict[str, PathLike[str]] | None = None,
        tables: dict[str, PathLike[str]] | None = None,
    ) -> None:
        self._internal = _server.Server(
            host=host,
            port=port,
            datasets=datasets,
            tables=tables,
        )

    def __enter__(self) -> Server:
        self._internal.__enter__()
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self._internal.__exit__(exc_type, exc_value, traceback)

    def url(self) -> str:
        """Get the URL of the server to which clients can connect."""
        return self._internal.url()

    def client(self) -> CatalogClient:
        return CatalogClient(self.url())
