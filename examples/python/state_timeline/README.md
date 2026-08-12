<!--[metadata]
title = "State timeline"
description = "Simulates a robot work cell and tours every feature of the state timeline view: transitions, custom styling, resets, and columnar logging."
tags = ["States", "API example"]
thumbnail = "https://static.rerun.io/state_timeline/88a787de865d94c0f93045deb1ae304060be16f7/480w.png"
thumbnail_dimensions = [480, 270]
channel = "main"
include_in_manifest = true
-->

This example simulates a robot work cell and demonstrates every feature of the state timeline view.

<!--
TODO(RR-4240): replace the thumbnail above and place a screenshot here.
Use `pixi run upload-image --help` for instructions.
-->

## Used Rerun types

[`StateChange`](https://www.rerun.io/docs/reference/types/archetypes/state_change), [`StateConfiguration`](https://www.rerun.io/docs/reference/types/archetypes/state_configuration), [`TextDocument`](https://www.rerun.io/docs/reference/types/archetypes/text_document)

## Logging and visualizing with Rerun

Discrete states are logged with the [`StateChange`](https://www.rerun.io/docs/reference/types/archetypes/state_change) archetype.
Each logged `StateChange` marks a transition into a new state, and the state timeline view displays these as horizontal colored lanes over time.

The example covers all features of the view:

- **State transitions**: each entity gets its own lane, and a state extends until the next transition (`robot/task`).
- **Custom styling**: [`StateConfiguration`](https://www.rerun.io/docs/reference/types/archetypes/state_configuration) maps raw state values to display labels and colors (`robot/task`).
- **Automatic styling**: without a configuration, raw values are used as labels and colors come from a built-in palette (`robot/gripper`).
- **Label fallback**: a `labels` array shorter than `values` falls back to the raw value for the missing entries (`robot/connection`).
- **State resets**: logging an empty string resets the state and leaves a gap in the lane (`robot/connection`).
- **Per-state visibility**: the `visible` array of `StateConfiguration` hides noisy states (`robot/diagnostics`).
- **Columnar logging**: batches of state changes can be sent in one call with `send_columns`; `null` entries reset the state and leave a gap, just like empty strings (`conveyor`).
- **Beyond strings**: any string, integer, float, or boolean component can drive a state lane, including custom components logged with `DynamicArchetype`. The blueprint maps them onto the `StateChange:state` slot of the state visualizer, so integer enums and boolean flags each get their own lane; `StateConfiguration` applies to them too, keyed by the displayed form of the value (`plc`).
- **Blueprint**: state timeline views are scoped with `origin` and filtered with `contents` entity path expressions.

## Run the code

To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:

```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
```

Install the necessary libraries specified in the requirements file:

```bash
pip install -e examples/python/state_timeline
```

To experiment with the provided example, simply execute the main Python script:

```bash
python -m state_timeline
```

If you wish to customize it, explore additional features, or save it, use the CLI with the `--help` option for guidance:

```bash
python -m state_timeline --help
```
