"""Log arbitrary data."""

import rerun as rr

rr.init("rerun_example_any_values", spawn=True)

rr.log(
    "any_values",
    rr.AnyValues(
        # URIs will become clickable links
        homepage="https://www.rerun.io",
        repository="https://github.com/rerun-io/rerun",
    )
    .with_field(
        rr.ComponentDescriptor("confidence", component_name=rr.components.ScalarBatch._COMPONENT_NAME), [1.2, 3.4, 5.6]
    )
    .with_field(
        rr.ComponentDescriptor("description", component_name=rr.components.TextBatch._COMPONENT_NAME), "Bla bla bla…"
    ),
)
