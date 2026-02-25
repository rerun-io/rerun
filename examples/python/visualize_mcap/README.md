<!--[metadata]
title = "Visualize MCAP"
tags = ["MCAP", "Component Mapping", "Blueprint", "Time series", "3D", "Robotics"]
channel = "release"
include_in_manifest = true
thumbnail = "https://static.rerun.io/visualize_mcap/2ab44dd2cd60b441b36020349541a64f2ca498eb/480w.png"
thumbnail_dimensions = [480, 254]
-->

Visualize Protobuf-encoded [MCAP](https://mcap.dev/) data in the Rerun Viewer using [component mapping](https://www.rerun.io/docs/concepts/visualization/visualizers-and-overrides) to extract and plot fields from raw messages—no custom deserialization code required.

<picture>
  <img src="https://static.rerun.io/visualize_mcap/2ab44dd2cd60b441b36020349541a64f2ca498eb/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/visualize_mcap/2ab44dd2cd60b441b36020349541a64f2ca498eb/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/visualize_mcap/2ab44dd2cd60b441b36020349541a64f2ca498eb/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/visualize_mcap/2ab44dd2cd60b441b36020349541a64f2ca498eb/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/visualize_mcap/2ab44dd2cd60b441b36020349541a64f2ca498eb/1200w.png">
</picture>

## Used Rerun types
- [`SeriesLines`](https://www.rerun.io/docs/reference/types/archetypes/series_lines)
- [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars)
- [`VisualizerComponentMapping`](https://www.rerun.io/docs/reference/types/datatypes/visualizer_component_mapping?speculative-link)
- [`TimeSeriesView`](https://www.rerun.io/docs/reference/types/views/time_series_view)
- [`Spatial3DView`](https://www.rerun.io/docs/reference/types/views/spatial3d_view)
- [`Spatial2DView`](https://www.rerun.io/docs/reference/types/views/spatial2d_view)

## Background

[MCAP](https://mcap.dev/) is a popular open-source container format for multimodal log data, widely used in robotics. Rerun can load MCAP files directly via [`log_file_from_path`](https://www.rerun.io/docs/reference/sdk/python#rerun.log_file_from_path) and has [built-in support for many popular robotics message types](https://www.rerun.io/docs/howto/logging-and-ingestion/mcap) such as images, point clouds, and transforms.

However, MCAP files often also contain domain-specific Protobuf messages (like `JointState` in this example) whose fields don't map to Rerun archetypes out of the box. **Component mapping** solves this: it lets you define, purely in the blueprint, how to extract fields from a source component and map them onto Rerun visualization types—using jq-like selectors—without writing any deserialization code.

## Component mapping

The core of this example is the blueprint-based component mapping. Each `JointState` Protobuf message contains `joint_positions` and `joint_names` arrays. Rather than parsing these in Python, the example uses [`VisualizerComponentMapping`](https://www.rerun.io/docs/reference/types/datatypes/visualizer_component_mapping?speculative-link) to tell the viewer how to extract them at display time:

```python
rr.SeriesLines().visualizer(
    mappings=[
        VisualizerComponentMapping(
            target="SeriesLines:names",
            source_kind=ComponentSourceKind.SourceComponent,
            source_component="schemas.proto.JointState:message",
            selector=".joint_names[]",
        ),
        VisualizerComponentMapping(
            target="Scalars:scalars",
            source_kind=ComponentSourceKind.SourceComponent,
            source_component="schemas.proto.JointState:message",
            selector=".joint_positions[]",
        ),
    ]
)
```

- **`source_component`** identifies the raw Protobuf component stored in the MCAP data (`schemas.proto.JointState:message`).
- **`selector`** is a jq-like expression (`.joint_positions[]`) that extracts the desired field.
- **`target`** specifies the Rerun archetype component to map it onto (`Scalars:scalars` for plotting, `SeriesLines:names` for labels).

This mapping is applied as a [visualizer override](https://www.rerun.io/docs/concepts/visualization/visualizers-and-overrides) on the relevant entity paths, so the raw Protobuf data is plotted as labeled time series in a [`TimeSeriesView`](https://www.rerun.io/docs/reference/types/views/time_series_view).

## Data

The MCAP data was generated from a modified version of the [Trossen Arm MuJoCo](https://github.com/TrossenRobotics/trossen_arm_mujoco) simulation and contains:

- Camera streams (overhead and wrist cameras for both arms)
- `JointState` messages with joint positions and names
- URDF robot descriptions with STL/OBJ meshes for 3D visualization

## Run the code

To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/visualize_mcap
```
Then, simply execute the main Python script:
```bash
python -m visualize_mcap
```
