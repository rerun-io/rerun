"""Set different types of indices."""

from datetime import datetime, timedelta, timezone

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_different_indices", spawn=True)

rr.set_index("frame_nr", sequence=42)
rr.set_index("elapsed", timedelta=12) # elapsed seconds
rr.set_index("time", datetime=1_741_017_564) # Seconds since unix epoch
rr.set_index("time", datetime=datetime.fromisoformat("2025-03-03T15:59:24"))
rr.set_index("precise_time", datetime=np.datetime64(1_741_017_564_987_654_321, "ns")) # Nanoseconds since unix epoch

# All following logged data will be timestamped with the above times:
rr.log("points", rr.Points2D([[0, 0], [1, 1]]))
