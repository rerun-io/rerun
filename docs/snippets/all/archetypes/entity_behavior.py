"""Configure interactivity & visibility of entities."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_entity_behavior", spawn=True)

# Use `EntityBehavior` to override visibility & interactivity of entities in the blueprint.
rr.send_blueprint(
    rrb.Spatial2DView(
        overrides={
            "hidden_subtree": rrb.EntityBehavior(visible=False),
            "hidden_subtree/not_hidden": rrb.EntityBehavior(visible=True),
            "non_interactive_subtree": rrb.EntityBehavior(interactive=False),
        }
    )
)

rr.log("hidden_subtree", rr.Points2D(positions=(0, 0), radii=0.5))
rr.log("hidden_subtree/also_hidden", rr.LineStrips2D(strips=[(-1, 1), (1, -1)]))
rr.log("hidden_subtree/not_hidden", rr.LineStrips2D(strips=[(1, 1), (-1, -1)]))
rr.log("non_interactive_subtree", rr.Boxes2D(centers=(0, 0), half_sizes=(1, 1)))
rr.log("non_interactive_subtree/also_non_interactive", rr.Boxes2D(centers=(0, 0), half_sizes=(0.5, 0.5)))
