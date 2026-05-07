# Log a `Status` together with a `StatusConfiguration` that customizes its display.

import rerun as rr

rr.init("rerun_example_status_configuration", spawn=True)

# Configure how each raw status value is displayed (label, color, visibility).
rr.log(
    "door",
    rr.StatusConfiguration(
        values=["open", "closed"],
        labels=["Open", "Closed"],
        colors=[0x4CAF50FF, 0xEF5350FF],
    ),
    static=True,
)

rr.set_time("step", sequence=0)
rr.log("door", rr.Status(status="open"))

rr.set_time("step", sequence=1)
rr.log("door", rr.Status(status="closed"))

rr.set_time("step", sequence=2)
rr.log("door", rr.Status(status="open"))
