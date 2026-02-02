"""
Demonstrates how to configure visualizer component mappings from blueprint.

⚠️TODO(#12600): The API for component mappings is still evolving, so this example may change in the future.
"""

from __future__ import annotations


import numpy as np
import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.datatypes import ComponentSourceKind, VisualizerComponentMapping

rr.init("rerun_example_component_mapping", spawn=True)

# Send plot data using send_columns.
times = np.arange(64)
rr.send_columns(
    "plot",
    indexes=[rr.TimeColumn("step", sequence=times)],
    columns=[
        # Regular scalar batch with a sin.
        *rr.Scalars.columns(scalars=np.sin(times / 10.0)),
        # Custom scalar batch with a cos using a custom component name.
        *rr.DynamicArchetype.columns(archetype="custom", components={"my_custom_scalar": np.cos(times / 10.0)}),
    ],
)

# Add a line series color to the store data.
rr.log("plot", rr.SeriesLines(colors=[255, 0, 0]), static=True)

# Create a blueprint with explicit component mappings
blueprint = rrb.Blueprint(
    rrb.TimeSeriesView(
        name="Component Mapping Demo",
        origin="/",
        # Set default color for series to blue.
        defaults=[rr.SeriesLines(colors=[0, 255, 0])],
        overrides={
            # Two line series visualizations for the "plot" entity:
            "plot": [
                # Red sine:
                # * set the name via an override
                # * explicitly use the view's default for color
                # * everything else uses the automatic component mappings, so it will pick up scalars from the store.
                rr.SeriesLines(names="sine (store)").visualizer(
                    mappings=[
                        VisualizerComponentMapping(
                            target="SeriesLines:colors",
                            source_kind=ComponentSourceKind.Default,
                        ),
                    ]
                ),
                # Blue cosine:
                # * source scalars from the custom component "custom:my_custom_scalar"
                # * set the name via an override
                # * everything else uses the automatic component mappings, so it will pick up colors from the view default.
                rr.SeriesLines(names="cosine (custom)").visualizer(
                    mappings=[
                        # Map scalars to the custom component.
                        VisualizerComponentMapping(
                            target="Scalars:scalars",
                            source_kind=ComponentSourceKind.SourceComponent,
                            source_component="custom:my_custom_scalar",  # Map from custom component
                        ),
                    ]
                ),
            ],
        },
    ),
)

rr.send_blueprint(blueprint)
