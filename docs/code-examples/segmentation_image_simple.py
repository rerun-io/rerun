"""Create and log a segmentation image."""

import numpy as np
import rerun as rr
import rerun.experimental as rr2

# Create a segmentation image
image = np.zeros((8, 12), dtype=np.uint8)
image[0:4, 0:6] = 1
image[4:8, 6:12] = 2

rr.init("rerun_example_segmentation_image", spawn=True)

# Assign a label and color to each class
rr2.log("/", rr2.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]))

# TODO(#2792): SegmentationImage archetype
rr.log_segmentation_image("image", np.array(image))
