# Log a `Status`.

import rerun as rr

rr.init("rerun_example_status", spawn=True)

rr.set_time("step", sequence=0)
rr.log("door", rr.Status(status="open"))

rr.set_time("step", sequence=1)
rr.log("door", rr.Status(status="closed"))

rr.set_time("step", sequence=2)
rr.log("door", rr.Status(status="open"))
