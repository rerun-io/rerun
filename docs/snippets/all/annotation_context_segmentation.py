"""Log a segmentation image with annotations."""

import numpy as np
import rerun as rr

rr.init("rerun_example_annotation_context_segmentation", spawn=True)

# Create a simple segmentation image
image = np.zeros((200, 300), dtype=np.uint8)
image[50:100, 50:120] = 1
image[100:180, 130:280] = 2

# Log an annotation context to assign a label and color to each class
rr.log("segmentation", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]), static=True)

rr.log("segmentation/image", rr.SegmentationImage(image))
