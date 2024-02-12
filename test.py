from __future__ import annotations

import numpy as np
import rerun as rr

rr.init("test", spawn=True)

rr.log("images/image1", rr.Image(np.random.rand(100, 100, 3)))
rr.log("images/image2", rr.Image(np.random.rand(100, 100, 3)))
rr.log("images/image3", rr.Image(np.random.rand(100, 100, 3)))
