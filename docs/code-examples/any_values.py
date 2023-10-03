"""Log a arbitrary data."""
import rerun as rr

rr.init("rerun_example_any_values", spawn=True)

# add custom component to an archetype
rr.log(
    "archetype_any_values",
    rr.Points2D([[0, 0], [1, 3], [2, 0]], radii=0.3),
    rr.AnyValues(
        foo=[1.2, 3.4, 5.6],
        bar="hello world",
    ),
)

# log an entirely custom set of components
rr.log(
    "any_values",
    rr.AnyValues(
        foo=[1.2, 3.4, 5.6],
        bar="hello world",
    ),
)
