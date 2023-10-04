"""Log arbitrary data."""
import rerun as rr

rr.init("rerun_example_any_values", spawn=True)

rr.log(
    "any_values",
    rr.AnyValues(
        foo=[1.2, 3.4, 5.6],
        bar="hello world",
    ),
)
