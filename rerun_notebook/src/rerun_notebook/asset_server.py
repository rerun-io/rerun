from __future__ import annotations

import http.server
import socketserver

from . import WIDGET_PATH

resource_data: bytes | None = None


class AssetHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)

    def do_GET(self):
        if self.path == "/widget.js":  # remap this path
            self.send_response(200)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Content-type", "text/javascript")
            self.end_headers()
            if resource_data is not None:
                self.wfile.write(resource_data)
        else:
            # Serve other requests normally
            self.send_error(404, "File Not Found")

    def log_message(self, format, *args):
        # Disable logging
        return


def serve_assets(bind_address: str = "localhost", port: int = 0, background=False) -> socketserver._AfInetAddress:
    global resource_data
    if resource_data is None:
        with open(WIDGET_PATH, "rb") as f:
            resource_data = f.read()

    httpd = socketserver.TCPServer((bind_address, port), AssetHandler)

    bound_addr = httpd.server_address
    print(f"Serving rerun notebook assets at http://{bound_addr[0]}:{bound_addr[1]}")

    if background:
        import threading

        thread = threading.Thread(target=httpd.serve_forever)
        thread.daemon = True
        thread.start()
    else:
        httpd.serve_forever()

    return bound_addr
