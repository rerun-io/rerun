"""Log arbitrary data along an archetype."""
import rerun as rr

rr.init("rerun_example_any_values_archetype", spawn=True)

rr.log(
    "any_values",
    rr.Points2D([[0, 0], [1, 3], [2, 0]], radii=0.3),
    rr.AnyValues(
        foo=[1.2, 3.4, 5.6],
        bar="hello world",
    ),
)
