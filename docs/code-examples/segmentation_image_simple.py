"""Create and log a segmentation image."""

import numpy as np
import rerun as rr

# Create a segmentation image
image = np.zeros((200, 300), dtype=np.uint8)
image[50:100, 50:120] = 1
image[100:180, 130:280] = 2

rr.init("rerun_example_segmentation_image", spawn=True)

# Assign a label and color to each class
rr.log_annotation_context(
    "/",
    [
        rr.ClassDescription(info=rr.AnnotationInfo(1, "red", (255, 0, 0))),
        rr.ClassDescription(info=rr.AnnotationInfo(2, "green", (0, 255, 0))),
    ],
)

rr.log_segmentation_image("image", np.array(image))
