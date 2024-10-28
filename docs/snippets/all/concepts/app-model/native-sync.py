#!/usr/bin/env python3

import rerun as rr

rr.init("rerun_example_native_sync")

# Connect to the Rerun TCP server using the default address and
# port: localhost:9876
rr.connect_tcp()

# Log data as usual, thereby pushing it into the TCP socket.
while True:
    rr.log("/", rr.TextLog("Logging things..."))
