# Log a `StateChange`.

import rerun as rr

rr.init("rerun_example_state_change", spawn=True)

rr.set_time("step", sequence=0)
rr.log("door", rr.StateChange(state="open"))

rr.set_time("step", sequence=1)
rr.log("door", rr.StateChange(state="closed"))

rr.set_time("step", sequence=2)
rr.log("door", rr.StateChange(state="open"))
