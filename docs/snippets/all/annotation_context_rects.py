import rerun as rr

rr.init("rerun_example_annotation_context_rects", spawn=True)

# Log an annotation context to assign a label and color to each class
rr.log("/", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]), timeless=True)

# Log a batch of 2 rectangles with different `class_ids`
rr.log("detections", rr.Boxes2D(mins=[[-2, -2], [0, 0]], sizes=[[3, 3], [2, 2]], class_ids=[1, 2]))

# Log an extra rect to set the view bounds
rr.log("bounds", rr.Boxes2D(half_sizes=[2.5, 2.5]))
