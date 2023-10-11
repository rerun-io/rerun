import numpy as np
import rerun as rr

rr.init("rerun_example_annotation_context_segmentation", spawn=True)

# Create a simple segmentation image
image = np.zeros((8, 12), dtype=np.uint8)
image[0:4, 0:6] = 1
image[4:8, 6:12] = 2

# Log an annotation context to assign a label and color to each class
rr.log("segmentation", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]), timeless=True)

rr.log("segmentation/image", rr.SegmentationImage(image))
