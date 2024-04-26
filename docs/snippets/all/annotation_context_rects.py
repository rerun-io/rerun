import rerun as rr
import rerun.blueprint as rrb

blueprint = rrb.Spatial2DView(background=[128, 0, 0], visual_bounds=rrb.VisualBounds(min=[-4.5, -2.5], max=[2.5, 2.5]))
rr.init("rerun_example_annotation_context_rects", spawn=True, default_blueprint=blueprint)

# Log an annotation context to assign a label and color to each class
rr.log("/", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]), static=True)

# Log a batch of 2 rectangles with different `class_ids`
rr.log("detections", rr.Boxes2D(mins=[[-2, -2], [0, 0]], sizes=[[3, 3], [2, 2]], class_ids=[1, 2]))
