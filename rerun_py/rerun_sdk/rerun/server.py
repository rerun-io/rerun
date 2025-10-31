from __future__ import annotations

import socket
from typing import TYPE_CHECKING

from rerun_bindings import _ServerInternal

from .catalog import CatalogClient

if TYPE_CHECKING:
    from os import PathLike
    from types import TracebackType


class Server:
    """
    A Rerun server instance.

    This class allows you to start and manage a Rerun server programmatically.
    The server hosts recordings and serves them via HTTP, and provides access to
    the catalog through a client connection.
    When the object goes out of scope the server is automatically shut down.

    The server can be used as a context manager, which will automatically shut down
    the server when exiting the context.

    Example
    -------
    ```python
    import rerun as rr

    # Start a server with some datasets
    with rr.Server(port=9876, datasets={"my_data": "path/to/data.rrd"}) as server:
        client = server.client()

        # Use the client to interact with the catalog
        datasets = client.datasets()
    ```

    """

    def __init__(
        self,
        *,
        address: str = "0.0.0.0",
        port: int | None = None,
        datasets: dict[str, PathLike[str]] | None = None,
        tables: dict[str, PathLike[str]] | None = None,
    ) -> None:
        """
        Create a new Rerun server instance and start it.

        The server will host recordings and serve them via HTTP. If datasets are provided,
        they will be loaded and made available when the server starts.

        Parameters
        ----------
        address:
            The address to bind the server to.
        port:
            The port to bind the server to, or `None` to select a random available port.
        datasets:
            Optional dictionary mapping dataset names to their file paths.
            These datasets will be loaded and available when the server starts.
        tables:
            Optional dictionary mapping table names to lance file paths,
            which will be loaded and made available when the server starts.

        """

        # Select a random open port if none is specified
        resolved_port: int
        if port is None:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
                s.bind(("", 0))
                resolved_port = s.getsockname()[1]
        else:
            resolved_port = port

        self._internal = _ServerInternal(
            address=address,
            port=resolved_port,
            datasets={name: str(path) for name, path in (datasets or {}).items()},
            tables={name: str(path) for name, path in (tables or {}).items()},
        )

    def client(self) -> CatalogClient:
        """
        Get a CatalogClient connected to this server.

        The client can be used to interact with the server's catalog, including
        querying datasets and tables.

        Note: the `datafusion` package is required to use the client. The client
        initialization will fail with an error if the package is not installed.

        Returns
        -------
        CatalogClient
            A client for interacting with the server's catalog.

        Raises
        ------
        RuntimeError
            If the server is not running.

        """
        if not self._internal.is_running():
            raise RuntimeError("Cannot create client: server is not running.")

        return CatalogClient(self._internal.address(), token=None)

    def is_running(self) -> bool:
        """
        Check if the server is currently running.

        Returns
        -------
        bool
            `True` if the server is running, `False` otherwise.

        """
        return self._internal.is_running()

    def shutdown(self) -> None:
        """
        Stop the server.

        After calling this method, the server will no longer be accessible.

        Raises
        ------
        RuntimeError
            If the server is not running.

        """
        self._internal.shutdown()

    def __enter__(self) -> Server:
        """Enter the context manager, returning the server instance."""
        return self

    def __exit__(
        self, exc_type: type[BaseException] | None, exc_value: BaseException | None, traceback: TracebackType | None
    ) -> None:
        """Exit the context manager, shutting down the server."""
        self._internal.shutdown()
