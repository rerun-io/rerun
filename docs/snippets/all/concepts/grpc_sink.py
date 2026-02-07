"""Create and set a GRPC sink."""

import rerun as rr

rr.init("rerun_example_grpc_sink")

# The default URL is `rerun+http://127.0.0.1:9876/proxy`
# This can be used to connect to a viewer on a different machine
rr.set_sinks(rr.GrpcSink("rerun+http://127.0.0.1:9876/proxy"))
