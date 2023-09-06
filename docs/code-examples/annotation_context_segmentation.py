import numpy as np
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_annotation_context_segmentation", spawn=True)

# Create a simple segmentation image
image = np.zeros((200, 300), dtype=np.uint8)
image[50:100, 50:120] = 1
image[100:180, 130:280] = 2

# Log an annotation context to assign a label and color to each class
rr2.log("segmentation", rr2.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]))

rr.log_segmentation_image("segmentation/image", np.array(image))
