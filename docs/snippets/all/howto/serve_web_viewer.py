"""Demonstrates how to log data to a gRPC server and connect the web viewer to it."""

import time

import rerun as rr

rr.init("rerun_example_serve_web_viewer")
# Start a gRPC server and use it as log sink.
server_uri = rr.serve_grpc()

# Connect the web viewer to the gRPC server and open it in the browser
rr.serve_web_viewer(connect_to=server_uri)

# Log some data to the gRPC server.
rr.log("data", rr.Boxes3D(half_sizes=[2.0, 2.0, 1.0]))

# Keep server running. If we cancel it too early, data may never arrive in the browser.
try:
    while True:
        time.sleep(1)
except KeyboardInterrupt:
    print("\nShutting down server…")
