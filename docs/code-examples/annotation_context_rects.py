import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_annotation_context_rects", spawn=True)

# Log an annotation context to assign a label and color to each class
rr2.log("/", rr2.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]))

# Log a batch of 2 rectangles with different `class_ids`
rr2.log("detections", rr2.Boxes2D(mins=[[-2, -2], [0, 0]], sizes=[[3, 3], [2, 2]], class_ids=[1, 2]))

# Log an extra rect to set the view bounds
rr2.log("bounds", rr2.Boxes2D(half_sizes=[2.5, 2.5]))
