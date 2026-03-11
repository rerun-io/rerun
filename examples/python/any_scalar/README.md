<!--[metadata]
title = "Any scalar"
tags = ["Any scalar", "Plotting", "DynamicArchetype"]
thumbnail = "https://static.rerun.io/any_scalar_example_market/4076a99f7fd5912af93258aa0c6c775a96f8b8e7/480w.png"
thumbnail_dimensions = [480, 259]
channel = "nightly"
include_in_manifest = true
-->
<!-- Andreas: I've changed this sample's channel in the 0.30.2 patch release to nightly since we ran into some problems with it on the Viewer's dashboard which I want to investigate separately. -->

<iframe width="560" height="315" src="https://www.youtube.com/embed/G9Xxf0sNYcQ?si=jfb-WrY9WrFGh6mB" title="Any Scalar Example" frameborder="0" allow="accelerometer; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share" referrerpolicy="strict-origin-when-cross-origin" allowfullscreen></iframe>

*A 6-minute narrated walkthrough of using the Rerun UI to plot arbitrary scalar data from a dataset (MCAP) is available on [Youtube](https://www.youtube.com/embed/G9Xxf0sNYcQ?si=jfb-WrY9WrFGh6mB).*

## Overview

This example demonstrates how to visualize arbitrary data, even when it was not logged with specific Rerun-semantics. With the **"Any Scalar"** feature, you can log complex data structures (like dictionaries or structs) once and use **Selectors** in the Blueprint to "pick" which internal fields to plot.

**Key Benefits:**

* **Decoupled Logging:** You no longer need to log separate scalar entities for every value you want to graph.
* **Selective Visualization:** Use a single data stream to power multiple different views by targeting specific component fields (e.g., `.position` or `.close`).

---

## Run the code

To run this example, make sure you have the [required Python version](https://ref.rerun.io/docs/python/main/common#supported-python-versions), the Rerun repository checked out and the latest SDK installed:

```sh
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```

Install the necessary libraries specified in the requirements file:

```sh
pip install -e examples/python/any_scalar
```

To experiment with the provided example, simply execute the main Python script:

```sh
python -m any_scalar --demo robotics # Simulated PID control
python -m any_scalar --demo market   # Real-time stock performance
```

If you wish to explore additional features, use the CLI with the `--help` option for guidance:
```sh
python -m any_scalar --help
```

---

## Guided demos

### 1. Robotics: PID controller telemetry

**Goal:** Visualize a control loop's internal state without logging separate scalars for every field.

In `robotics_demo.py`, we simulate a joint controller. Instead of logging `error`, `effort`, and `position` as individual Rerun entities, we log a single **Telemetry struct** per time step.

**Tutorial highlights:**

- **Decoupled logging:** We log one `ControllerTelemetry` object. Later, in the Blueprint, we "pick" which parts to see.
- **Visualizer mapping:** Notice how the `Error` field is plotted twice: once as a **Line** (to see trends) and once as **Points** (to see individual sample timing).
- **Step interpolation:** The `Effort` signal uses `StepAfter` interpolation, which accurately reflects how a digital controller holds its output constant between steps.
- **Boolean plotting:** The `is_stable` flag is visualized as a step-function scalar (0/1).

What you should see when running `python -m any_scalar --demo robotics`:

<picture>
  <img src="https://static.rerun.io/any_scalar_example_robotics/f665e28b471f5b7b575c14e0a7fe11b10f636b88/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/any_scalar_example_robotics/f665e28b471f5b7b575c14e0a7fe11b10f636b88/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/any_scalar_example_robotics/f665e28b471f5b7b575c14e0a7fe11b10f636b88/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/any_scalar_example_robotics/f665e28b471f5b7b575c14e0a7fe11b10f636b88/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/any_scalar_example_robotics/f665e28b471f5b7b575c14e0a7fe11b10f636b88/1200w.png">
</picture>

### 2. Market data: relative performance

**Goal:** Compare multiple live data streams (tickers) using a centralized selector.

In `market_demo.py`, we fetch real stock data. We log the raw prices and a "normalized" % change field.

**Tutorial highlights:**

- **Dynamic normalization:** We log the price relative to the morning opening.
- **Dynamic archetype:** We log the stock data as a dictionary using `rerun.DynamicArchetype`.
- **Selectors:** We use [`jq`](https://jqlang.org/)-style selectors, like `.prices.normalized` and `.prices.close`, to power different parts of the dashboard.

What you should see when running `python -m any_scalar --demo market`:

<picture>
  <img src="https://static.rerun.io/any_scalar_example_market/4076a99f7fd5912af93258aa0c6c775a96f8b8e7/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/any_scalar_example_market/4076a99f7fd5912af93258aa0c6c775a96f8b8e7/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/any_scalar_example_market/4076a99f7fd5912af93258aa0c6c775a96f8b8e7/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/any_scalar_example_market/4076a99f7fd5912af93258aa0c6c775a96f8b8e7/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/any_scalar_example_market/4076a99f7fd5912af93258aa0c6c775a96f8b8e7/1200w.png">
</picture>

### 3. Load datasets directly into the viewer

**Goal:** Plot values from dataset files without writing any code.

Because Rerun can now plot **Any Scalar**, you can drag an `.mcap` or `.rrd` file into the viewer and create a `Time Series` view. Use the UI in the viewer to drill into nested ROS messages or telemetry logs and start plotting immediately.

> [!TIP]
> **Watch the video at the top of this page** to see a step-by-step walkthrough of how to use the UI to plot any field from an MCAP/RRD file.

---

## The "Magic": component mapping

### What is "Any Scalar"?

Traditionally, to plot a graph, you had to log data specifically as one of Rerun's `Scalar` archetypes. With **Any Scalar**, you can log complex blobs (Dictionaries, TypedDicts, Arrow Structs) and Rerun will let you "map" internal fields to visualizers.

### Selectors (jq-style)

Rerun uses a path syntax inspired by [`jq`](https://jqlang.org/) to reach into your data:
- `.state.position` -> reaches into the `state` dict and finds `position`.
- `.prices.normalized` -> pulls the calculated performance from the market tick.

### Benefits

1. **Developer velocity:** Log your entire state object once; decide what to plot later in the UI.
2. **Smaller files:** Less metadata overhead than logging 50 separate entities.
3. **Flexibility:** Change what you are visualizing in the Blueprint without restarting your simulation or re-running your data pipeline.

---

## Resources

- [Customize views](https://rerun.io/docs/concepts/visualization/customize-views)
- [Plot any scalar](https://rerun.io/docs/howto/visualization/plot-any-scalar)
- [Component Mappings Guide](https://rerun.io/docs/howto/visualization/component-mappings)

---

## Used Rerun types

[`DynamicArchetype`](https://ref.rerun.io/docs/python/stable/common/custom_data/#rerun.dynamic_archetype.DynamicArchetype), [`SeriesLines`](https://rerun.io/docs/reference/types/archetypes/series_lines), [`SeriesPoints`](https://rerun.io/docs/reference/types/archetypes/series_points)

