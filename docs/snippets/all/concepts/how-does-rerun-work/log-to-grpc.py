import rerun as rr

rr.init("rerun_example_log_to_grpc")

# Connect to the Rerun gRPC server using the default address and url: rerun+http://localhost:9876/proxy
rr.connect_grpc()

# Log data as usual, thereby pushing it into the gRPC connection.
while True:
    rr.log("/", rr.TextLog("Logging things…"))
