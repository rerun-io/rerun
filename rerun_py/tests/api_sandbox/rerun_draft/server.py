from __future__ import annotations

from rerun import server as _server

from .catalog import CatalogClient


class Server:
    __init__ = _server.Server.__init__
    address = _server.Server.address
    is_running = _server.Server.is_running
    shutdown = _server.Server.shutdown
    __enter__ = _server.Server.__enter__
    __exit__ = _server.Server.__exit__

    def client(self) -> CatalogClient:
        return CatalogClient(address=self.address())
