"""Place a point at the center of the top-left pixel of an image."""

import numpy as np

import rerun as rr

rr.init("rerun_example_spatial2d_pixel_centers", spawn=True)

image = np.array([[64, 128], [192, 255]], dtype=np.uint8)
rr.log("camera/image", rr.Image(image))
rr.log("camera/point", rr.Points2D([[0.5, 0.5]], colors=[255, 0, 0]))
