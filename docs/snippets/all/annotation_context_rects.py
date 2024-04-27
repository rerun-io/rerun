import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_annotation_context_rects", spawn=True)

# Log an annotation context to assign a label and color to each class
rr.log("/", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]), static=True)

# Log a batch of 2 rectangles with different `class_ids`
rr.log("detections", rr.Boxes2D(mins=[[-2, -2], [0, 0]], sizes=[[3, 3], [2, 2]], class_ids=[1, 2]))

# Create a Spatial3D View
blueprint = rrb.Blueprint(
    rrb.Spatial3DView(
        origin="/points",
        background=[80, 80, 80],
    )
)

rr.send_blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds(min=[-4.5, -2.5], max=[2.5, 2.5])))
