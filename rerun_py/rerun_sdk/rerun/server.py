from __future__ import annotations

from os import PathLike
from typing import TYPE_CHECKING

from rerun_bindings import ServerInternal


from .catalog import CatalogClient


class Server:
    """
    A Rerun server instance.

    This class allows you to start and manage a Rerun server programmatically.
    """

    def __init__(
        self,
        address: str | None = None,
        port: int | None = None,
        datasets: dict[str, PathLike] | None = None,
    ) -> None:
        """
        Initialize and start a Rerun server.

        Parameters
        ----------
        address : str | None
            The address on which the server should listen. If None, the default address `0.0.0.0` will be used.

        port : int | None
            The port on which the server should listen. If None, the default port `51234` will be used.

        datasets : dict[str, PathLike] | None
            A mapping of dataset names to their paths. If None, no datasets will be loaded.

        """

        self._internal = ServerInternal(
            address=address, port=port, datasets={name: str(path) for name, path in (datasets or {}).items()}
        )

    def client(self) -> CatalogClient:
        """
        Get a CatalogClient connected to this server.

        Returns
        -------
        CatalogClient
            A client for interacting with the server's catalog.

        """
        return CatalogClient(self._internal.address(), token=None)
