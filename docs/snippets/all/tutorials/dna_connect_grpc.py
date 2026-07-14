"""DNA-abacus example, connecting to a separately-running viewer over gRPC."""

import rerun as rr

rr.init("rerun_example_dna_abacus")
rr.connect_grpc()  # connect to the viewer running at the default URL

# … log data as in the spawn-based example …
