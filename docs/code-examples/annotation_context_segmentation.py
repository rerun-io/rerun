import numpy as np
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_annotation_context_segmentation", spawn=True)

# Create a simple segmentation image
image = np.zeros((20, 30), dtype=np.uint8)
image[5:10, 5:12] = 1
image[10:18, 13:28] = 2

# Log an annotation context to assign a label and color to each class
rr2.log("segmentation", rr2.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]))

rr.log_segmentation_image("segmentation/image", np.array(image))
