---
title: Building blueprints programmatically
order: 600
description: Lay out the Viewer from Python or Rust code with the Blueprint API
---

You can define [Blueprints](../blueprints.md) in code using the Blueprint API in Python or Rust.

This guide configures the viewport for individual recordings.
To configure catalog table columns or embedded segment previews instead, see [Configure table layouts and segment previews](configure-table-blueprints.md).

Most of this guide uses Python.
For Rust, see [Rust Blueprint API](#rust-blueprint-api) below.
C++ has no Blueprint API yet, but can [load blueprint files](#loading-blueprints-from-any-language).
If your blueprint has no effect, see [Why is my blueprint ignored?](../blueprints.md#why-is-my-blueprint-ignored).

## Getting started example

This walkthrough demonstrates the Blueprint API using stock market data. We'll start simple and progressively build more complex layouts.
It logs synthetic prices; the [`blueprint_stocks` example](https://github.com/rerun-io/rerun/tree/latest/examples/python/blueprint_stocks) fetches real ones from Yahoo Finance.

### Setup

First, create a virtual environment and install dependencies:

**Linux/Mac:**

```bash
python -m venv venv
source venv/bin/activate
pip install rerun-sdk numpy
```

**Windows:**

```bash
python -m venv venv
.\venv\Scripts\activate
pip install rerun-sdk numpy
```

### Basic script

Create `stocks.py`, which logs five days of prices for five stocks:

snippet: howto/visualization/blueprint_stocks[script]

Run the script:

```bash
python stocks.py
```

Without a blueprint, the heuristic layout may not be ideal:

<picture>
  <img src="https://static.rerun.io/blueprint_tutorial_no_blueprint/b7341f41683825f4186d661af509f8da03dc4ed1/full.png" alt="Heuristic layout without a blueprint: one crowded stocks plot and five text views with company info in a grid">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_tutorial_no_blueprint/b7341f41683825f4186d661af509f8da03dc4ed1/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_tutorial_no_blueprint/b7341f41683825f4186d661af509f8da03dc4ed1/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_tutorial_no_blueprint/b7341f41683825f4186d661af509f8da03dc4ed1/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_tutorial_no_blueprint/b7341f41683825f4186d661af509f8da03dc4ed1/1200w.png">
</picture>

## Creating a simple view

Add a blueprint to the end of the script:

snippet: howto/visualization/blueprint_stocks[one_stock]

The `origin` parameter scopes the view to a specific subtree. Now you'll see just the AAPL data:

<picture>
  <img src="https://static.rerun.io/blueprint_tutorial_one_stock/bda8f536306f9d9eb1b2aafe8bd8aceb746c2e0c/full.png" alt="Time series view showing only AAPL stock prices for five days, with red peak markers, next to the blueprint and selection panels">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock/bda8f536306f9d9eb1b2aafe8bd8aceb746c2e0c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock/bda8f536306f9d9eb1b2aafe8bd8aceb746c2e0c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock/bda8f536306f9d9eb1b2aafe8bd8aceb746c2e0c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock/bda8f536306f9d9eb1b2aafe8bd8aceb746c2e0c/1200w.png">
</picture>

## Controlling panel state

You can control which panels are visible:

snippet: howto/visualization/blueprint_stocks[hide_panels]

<picture>
  <img src="https://static.rerun.io/blueprint_tutorial_one_stock_hide_panels/41d3f42d2e33bcaec33b27e98752eddb17352c0f/full.png" alt="AAPL time series view filling the Viewer with the selection and time panels collapsed">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_hide_panels/41d3f42d2e33bcaec33b27e98752eddb17352c0f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_hide_panels/41d3f42d2e33bcaec33b27e98752eddb17352c0f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_hide_panels/41d3f42d2e33bcaec33b27e98752eddb17352c0f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_hide_panels/41d3f42d2e33bcaec33b27e98752eddb17352c0f/1200w.png">
</picture>

## Combining multiple views

Use containers to combine multiple views. The `Vertical` container stacks views, and `row_shares` controls relative sizing:

snippet: howto/visualization/blueprint_stocks[one_stock_with_info]

<picture>
  <img src="https://static.rerun.io/blueprint_tutorial_one_stock_and_info/9fbf481aaf9da399718d8afb9f64b9364bb34268/full.png" alt="Vertical layout with a small Info text view for Apple Inc. stacked above a larger AAPL price chart">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_and_info/9fbf481aaf9da399718d8afb9f64b9364bb34268/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_and_info/9fbf481aaf9da399718d8afb9f64b9364bb34268/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_and_info/9fbf481aaf9da399718d8afb9f64b9364bb34268/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_and_info/9fbf481aaf9da399718d8afb9f64b9364bb34268/1200w.png">
</picture>

## Specifying view contents

The `contents` parameter provides fine-grained control over what appears in a view. You can include data from multiple sources:

snippet: howto/visualization/blueprint_stocks[compare_two]

<picture>
  <img src="https://static.rerun.io/blueprint_tutorial_comare_two/0ac7d7d02bebb433828aec16a085716951740dff/full.png" alt="Time series view titled META vs MSFT comparing the two stock price lines, blue for META and orange for MSFT">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_tutorial_comare_two/0ac7d7d02bebb433828aec16a085716951740dff/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_tutorial_comare_two/0ac7d7d02bebb433828aec16a085716951740dff/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_tutorial_comare_two/0ac7d7d02bebb433828aec16a085716951740dff/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_tutorial_comare_two/0ac7d7d02bebb433828aec16a085716951740dff/1200w.png">
</picture>

## Filtering with expressions

Content expressions can include or exclude subtrees using wildcards. They can reference `$origin` and use `/**` to match entire subtrees:

snippet: howto/visualization/blueprint_stocks[no_peaks]

<picture>
  <img src="https://static.rerun.io/blueprint_tutorial_one_stock_no_peaks/d53c5294e3ee118c5037d1b3480176ef49cb2071/full.png" alt="AAPL time series view with the peak markers filtered out, leaving only the daily price lines">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_no_peaks/d53c5294e3ee118c5037d1b3480176ef49cb2071/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_no_peaks/d53c5294e3ee118c5037d1b3480176ef49cb2071/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_no_peaks/d53c5294e3ee118c5037d1b3480176ef49cb2071/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_tutorial_one_stock_no_peaks/d53c5294e3ee118c5037d1b3480176ef49cb2071/1200w.png">
</picture>

See [Entity path filter](../view-concepts/entity-path-filter.md) for complete expression syntax.

## Programmatic layout generation

Since blueprints are Python code, you can generate them dynamically. This example creates a grid with one row per stock symbol:

snippet: howto/visualization/blueprint_stocks[grid]

<picture>
  <img src="https://static.rerun.io/blueprint_tutorial_grid/b9c41481818f9028d75df6076c62653989a02c66/full.png" alt="Grid of views with one row per stock symbol (AAPL, AMZN, GOOGL, META, MSFT): an info panel followed by a chart for each of five days">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/blueprint_tutorial_grid/b9c41481818f9028d75df6076c62653989a02c66/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/blueprint_tutorial_grid/b9c41481818f9028d75df6076c62653989a02c66/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/blueprint_tutorial_grid/b9c41481818f9028d75df6076c62653989a02c66/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/blueprint_tutorial_grid/b9c41481818f9028d75df6076c62653989a02c66/1200w.png">
</picture>

## Saving blueprints from code

You can save programmatically-created blueprints to `.rbl` files:

snippet: howto/visualization/save_blueprint

### Loading blueprints from any language

Existing blueprint files (e.g. created with the Python SDK or saved from the viewer) can be programmatically loaded into Rerun.
This is particularly useful when using the C++ SDK, since the Blueprint API is not available for C++:

snippet: howto/visualization/load_blueprint

This works using the `log_file_from_path` API, which allows you to log any file that contains data that Rerun understands — in this case, blueprint data.

API reference:

-   [🐍 Python `log_file_from_path`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log_file_from_path)
-   [🦀 Rust `log_file_from_path`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_file_from_path)
-   [🌊 C++ `log_file_from_path`](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#a20798d7ea74cce5c8174e5cacd0a2c47)


See the [Blueprint API Reference](https://ref.rerun.io/docs/python/stable/common/blueprint_apis/) for complete details.

## Advanced customization

Each view has its own blueprint properties.
For example:

snippet: howto/visualization/blueprint_view_options[spatial3d_properties]

snippet: howto/visualization/blueprint_view_options[time_series_properties]

See the [view guides](../view-types.md), including [3D camera settings](../view-types/3d.md#control-the-3d-camera) and [time series axes and ranges](../view-types/time-series.md#axis-range-and-legend).
For component defaults and overrides shared by views, see [Customize views](../view-concepts/customize-views.md).

## Rust blueprint API

Rust has a builder API in the [`rerun::blueprint`](https://docs.rs/rerun/latest/rerun/blueprint/index.html) module.
It supports containers (`Horizontal`, `Vertical`, `Grid`, `Tabs`), all view classes, view origin and contents, component defaults and overrides, and the state of the panels.
Most view-specific properties (for example eye controls or visible time ranges) are only available in Python for now.

This example is adapted from [`examples/rust/blueprint`](https://github.com/rerun-io/rerun/tree/latest/examples/rust/blueprint):

snippet: howto/blueprint_grid

`with_blueprint` makes the blueprint active immediately.
`with_default_blueprint` only sets it as the default, which the user gets when they reset the blueprint.
To send a blueprint to a stream that already exists, use `blueprint.send(&rec, BlueprintActivation { make_active: true, make_default: true })`.

---

## Youtube overview

For a video introduction, watch the [blueprints overview on YouTube](https://www.youtube.com/watch?v=kxbkbFVAsBo).
The code below is a simplified version of the video's example: it sends each blueprint in turn, so running it shows only the last one.

snippet: howto/visualization/blueprint_overview
