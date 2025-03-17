"""Set different types of indices."""

from datetime import datetime

import numpy as np
import rerun as rr

rr.init("rerun_example_different_indices", spawn=True)

rr.set_time("frame_nr", sequence=42)
rr.set_time("elapsed", duration=12)  # elapsed seconds
rr.set_time("time", timestamp=1_741_017_564)  # Seconds since unix epoch
rr.set_time("time", timestamp=datetime.fromisoformat("2025-03-03T15:59:24"))
rr.set_time("precise_time", timestamp=np.datetime64(1_741_017_564_987_654_321, "ns"))  # Nanoseconds since unix epoch

# All following logged data will be timestamped with the above times:
rr.log("points", rr.Points2D([[0, 0], [1, 1]]))
