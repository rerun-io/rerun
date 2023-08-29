import rerun as rr

rr.init("rerun_example_annotation_context_rects", spawn=True)

# Log an annotation context to assign a label and color to each class
rr.log_annotation_context(
    "/",
    [
        rr.ClassDescription(info=rr.AnnotationInfo(1, "red", (255, 0, 0))),
        rr.ClassDescription(info=rr.AnnotationInfo(2, "green", (0, 255, 0))),
    ],
)

# Log a batch of 2 rectangles with different `class_ids`
rr.log_rects("detections", [[-2, -2, 3, 3], [0, 0, 2, 2]], class_ids=[1, 2], rect_format=rr.RectFormat.XYWH)

# Log an extra rect to set the view bounds
rr.log_rect("bounds", [0, 0, 5, 5], rect_format=rr.RectFormat.XCYCWH)
