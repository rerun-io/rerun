import numpy as np
import rerun as rr

rr.init("rerun-example-annotation_context_segmentation", spawn=True)

# Create a simple segmentation image
image = np.zeros((200, 300), dtype=np.uint8)
image[50:100, 50:120] = 1
image[100:180, 130:280] = 2

# Log an annotation context to assign a label and color to each class
rr.log_annotation_context(
    "segmentation",
    [
        rr.ClassDescription(info=rr.AnnotationInfo(1, "red", (255, 0, 0))),
        rr.ClassDescription(info=rr.AnnotationInfo(2, "green", (0, 255, 0))),
    ],
)

rr.log_segmentation_image("segmentation/image", np.array(image))
