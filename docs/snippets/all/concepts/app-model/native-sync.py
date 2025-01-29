#!/usr/bin/env python3

import rerun as rr

rr.init("rerun_example_native_sync")

# Connect to the Rerun gRPC server using the default address and url: http://localhost:1852
rr.connect_grpc()

# Log data as usual, thereby pushing it into the TCP socket.
while True:
    rr.log("/", rr.TextLog("Logging things..."))
