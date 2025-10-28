from __future__ import annotations

from typing import TYPE_CHECKING

from rerun_bindings import ServerInternal

from .catalog import CatalogClient

if TYPE_CHECKING:
    from os import PathLike


class Server:
    """
    A Rerun server instance.

    This class allows you to start and manage a Rerun server programmatically.
    The server hosts recordings and serves them via HTTP, and provides access to
    the catalog through a client connection.

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
        entries = client.all_entries()
    ```

    """

    def __init__(
        self,
        address: str | None = None,
        port: int | None = None,
        datasets: dict[str, PathLike] | None = None,
    ) -> None:
        """
        Create a new Rerun server instance and start it.

        The server will host recordings and serve them via HTTP. If datasets are provided,
        they will be loaded and made available when the server starts.

        Parameters
        ----------
        address:
            The address to bind the server to.
            Defaults to `"0.0.0.0"`.
        port:
            The port to bind the server to.
            Defaults to `51234`.
        datasets:
            Optional dictionary mapping dataset names to their file paths.
            These datasets will be loaded and available when the server starts.

        """

        self._internal = ServerInternal(
            address=address, port=port, datasets={name: str(path) for name, path in (datasets or {}).items()}
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

    def stop(self) -> None:
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

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        """Exit the context manager, shutting down the server."""
        self._internal.shutdown()
