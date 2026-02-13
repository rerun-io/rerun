"""Log arbitrary archetype data."""

import rerun as rr

rr.init("rerun_example_dynamic_archetype", spawn=True)

rr.log(
    "new_archetype",
    rr.DynamicArchetype(
        archetype="MyArchetype",
        components={
            # Using arbitrary Arrow data.
            "homepage": "https://www.rerun.io",
            "repository": "https://github.com/rerun-io/rerun",
        },
    )
    # Using Rerun's builtin components.
    .with_component_override("confidence", rr.components.ScalarBatch._COMPONENT_TYPE, [1.2, 3.4, 5.6])
    .with_component_override("description", rr.components.TextBatch._COMPONENT_TYPE, "Bla bla bla…"),
)
