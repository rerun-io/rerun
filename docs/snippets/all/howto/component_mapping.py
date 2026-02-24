"""
Demonstrates how to configure visualizer component mappings from blueprint.

⚠️TODO(#12600): The API for component mappings is still evolving, so this example may change in the future.
"""

from __future__ import annotations


import numpy as np
import pyarrow as pa
import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.datatypes import ComponentSourceKind, VisualizerComponentMapping


# region: nested_struct
def make_sigmoid_struct_array(steps: int) -> pa.StructArray:
    """Creates a StructArray with a `values` field containing sigmoid data.

    Note: We intentionally use float32 here to demonstrate that the data will be
    automatically cast to the correct type (float64) when resolved by the visualizer.
    """
    x = np.arange(steps, dtype=np.float32) / 10.0
    sigmoid_values = 1.0 / (1.0 + np.exp(-(x - 3.0)))
    return pa.StructArray.from_arrays([pa.array(sigmoid_values, type=pa.float32())], names=["values"])


# endregion: nested_struct


rr.init("rerun_example_component_mapping", spawn=True)

# Send plot data using send_columns.
times = np.arange(64)
rr.send_columns(
    "plot",
    indexes=[rr.TimeColumn("step", sequence=times)],
    columns=[
        # Regular scalar batch with a sin.
        *rr.Scalars.columns(scalars=np.sin(times / 10.0)),
        # region: custom_data
        # Custom scalar batch with a cos using a custom component name.
        *rr.DynamicArchetype.columns(archetype="custom", components={"my_custom_scalar": np.cos(times / 10.0)}),
        # Nested custom scalar batch with a sigmoid inside a struct.
        *rr.DynamicArchetype.columns(
            archetype="custom", components={"my_nested_scalar": make_sigmoid_struct_array(64)}
        ),
        # endregion: custom_data
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
            # Three line series visualizations for the "plot" entity:
            "plot": [
                # region: custom_value
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
                # endregion: custom_value
                # region: source_mapping
                # Green cosine:
                # * source scalars from the custom component "custom:my_custom_scalar"
                # * set the name via an override
                # * everything else uses the automatic component mappings, so it will pick up colors from the view default.
                # region: source_mapping
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
                # endregion: source_mapping
                # region: selector_mapping
                # Blue sigmoid:
                # * source scalars from a nested struct using a selector to extract the "values" field
                # * set the name and an explicit blue color via overrides
                rr.SeriesLines(names="sigmoid (nested)", colors=[0, 0, 255]).visualizer(
                    mappings=[
                        VisualizerComponentMapping(
                            target="Scalars:scalars",
                            source_kind=ComponentSourceKind.SourceComponent,
                            source_component="custom:my_nested_scalar",
                            selector=".values",
                        ),
                    ]
                ),
                # endregion: selector_mapping
            ],
        },
    ),
)

rr.send_blueprint(blueprint)
