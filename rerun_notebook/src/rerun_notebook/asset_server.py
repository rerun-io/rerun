from __future__ import annotations

import gzip
import http.server
import socketserver
from pathlib import Path
from typing import Any

from . import WASM_PATH, WIDGET_PATH


class _Asset:
    def __init__(self, path: str | Path, content_type: str, encode_gzip: bool = False) -> None:
        self.data = Path(path).read_bytes()
        self.headers = {
            "Content-Type": content_type,
        }

        if encode_gzip:
            self.data = gzip.compress(self.data)
            self.headers["Content-Encoding"] = "gzip"


assets: dict[str, _Asset] | None = None


class AssetHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args: Any, **kwargs: Any) -> None:
        super().__init__(*args, **kwargs)

    def do_HEAD(self) -> None:
        if self.path == "/widget.js":
            self._serve_HEAD("widget.js")
        elif self.path == "/re_viewer_bg.wasm":
            self._serve_HEAD("re_viewer_bg.wasm")
        else:
            self.send_error(404, "File not found")

    def do_GET(self) -> None:
        if self.path == "/widget.js":  # remap this path
            self._serve_GET("widget.js")
        elif self.path == "/re_viewer_bg.wasm":
            self._serve_GET("re_viewer_bg.wasm")
        else:
            # Serve other requests normally
            self.send_error(404, "File Not Found")

    def _serve_GET(self, name: str) -> None:
        if assets is None:
            self.send_error(500, "Resources not loaded")
            return

        asset = assets[name]
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET,HEAD,OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "*")
        for key, value in asset.headers.items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(asset.data)

    def _serve_HEAD(self, name: str) -> None:
        if assets is None:
            self.send_error(500, "Resources not loaded")
            return

        asset = assets[name]
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET,HEAD,OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "*")
        for key, value in asset.headers.items():
            self.send_header(key, value)
        self.end_headers()

    def log_message(self, format: str, *args: Any) -> None:
        # Disable logging
        return


def serve_assets(
    bind_address: str = "localhost", port: int = 0, background: bool = False
) -> socketserver._AfInetAddress:
    print("Starting asset server due to RERUN_NOTEBOOK_ASSET=serve-local")
    global assets
    if assets is None:
        print("Loading assets into memory...")
        assets = {
            "widget.js": _Asset(WIDGET_PATH, "text/javascript"),
            "re_viewer_bg.wasm": _Asset(WASM_PATH, "application/wasm", encode_gzip=True),
        }

    httpd = socketserver.TCPServer((bind_address, port), AssetHandler)
    bound_addr = httpd.server_address
    print(f"Serving rerun notebook assets at http://{str(bound_addr[0])}:{str(bound_addr[1])}")

    if background:
        import threading

        thread = threading.Thread(target=httpd.serve_forever)
        thread.daemon = True
        thread.start()
    else:
        httpd.serve_forever()

    return bound_addr
