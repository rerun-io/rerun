# Log a `StateChange` together with a `StateConfiguration` that customizes its display.

import rerun as rr

rr.init("rerun_example_state_configuration", spawn=True)

# Configure how each raw state value is displayed (label, color, visibility).
rr.log(
    "door",
    rr.StateConfiguration(
        values=["open", "closed"],
        labels=["Open", "Closed"],
        colors=[0x4CAF50FF, 0xEF5350FF],
    ),
    static=True,
)

rr.set_time("step", sequence=0)
rr.log("door", rr.StateChange(state="open"))

rr.set_time("step", sequence=1)
rr.log("door", rr.StateChange(state="closed"))

rr.set_time("step", sequence=2)
rr.log("door", rr.StateChange(state="open"))
