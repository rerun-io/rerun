"""Log a pinhole and a random image."""
import numpy as np
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_pinhole", spawn=True)
rng = np.random.default_rng(12345)

image = rng.uniform(0, 255, size=[3, 3, 3])
rr2.log("world/image", rr2.Pinhole(focal_length_px=3, width=3, height=3))
rr2.log("world/image", rr2.Image(image))
