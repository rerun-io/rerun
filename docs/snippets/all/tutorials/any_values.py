"""Log arbitrary data."""

import rerun as rr

rr.init("rerun_example_any_values", spawn=True)

rr.log(
    "any_values",
    rr.AnyValues(
        # Using arbitrary Arrow data.
        homepage="https://www.rerun.io",
        repository="https://github.com/rerun-io/rerun",
    )
    # Using Rerun's builtin components.
    .with_component("confidence", rr.components.ScalarBatch._COMPONENT_TYPE, [1.2, 3.4, 5.6])
    .with_component("description", rr.components.TextBatch._COMPONENT_TYPE, "Bla bla bla…"),
)
