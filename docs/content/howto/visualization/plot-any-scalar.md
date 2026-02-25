---
title: Plot any scalar
order: 600
---

Rerun can plot numerical data as a time series, even data that wasn't logged with Rerun semantics.
By remapping where a visualizer reads its inputs from, you can separate how you _model_ your data from how you _visualize_ it.
This is useful for plotting custom messages from MCAPs, or data logged via `AnyValues` and `DynamicArchetype`.
As a bonus, logging multiple scalars to the same entity can drastically reduce `.rrd` file sizes.

Each visualizer takes components as input and determines their values from various sources.
By configuring _component mappings_, you can control exactly where each input comes from.

The supported data types are:

- `Float32` and `Float64`
- `Int8`, `Int16`, `Int32`, and `Int64`
- `UInt8`, `UInt16`, `UInt32`, and `UInt64`
- `Boolean`
- Any of the above nested inside of [Arrow structs](https://arrow.apache.org/docs/format/Intro.html#struct).

For background on how visualizers resolve component values, see [Customize views](../../concepts/visualization/visualizers-and-overrides.md).

## Logging custom data

Use `DynamicArchetype` to send data with custom component names alongside regular Rerun data.
Flat arrays and Arrow `StructArray`s are both supported.

This is what the data looks like for the `/plot` entity:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mapping-data-overview/e42dc535b06c41e193b7b29f2e88aadd6506cafc/full.png" alt="Data overview for the /plot entity showing scalar, custom, and nested components">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mapping-data-overview/e42dc535b06c41e193b7b29f2e88aadd6506cafc/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mapping-data-overview/e42dc535b06c41e193b7b29f2e88aadd6506cafc/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mapping-data-overview/e42dc535b06c41e193b7b29f2e88aadd6506cafc/1024w.png">
</picture>

snippet: howto/component_mapping[custom_data]

## Remapping components

A visualizer can source its inputs from any component with a compatible datatype.
For example, the `SeriesLines` visualizer accepts any numerical data for its `Scalar` input.
This works with data from MCAP files, `AnyValues`, or `DynamicArchetype`.
Optional components like `Names` and `Colors` can be sourced similarly from arbitrary data.

The following remaps the `Scalars:scalars` input to read from `custom:my_custom_scalar` instead:

snippet: howto/component_mapping[source_mapping]

## Selectors for nested data

When your data lives inside an Arrow `StructArray`, use a _selector_ to extract a specific field.
Selectors use a `jq`-inspired syntax (e.g. `.values` to select the `values` field).

Data types are automatically cast when compatible. For example, `Float32` data will be cast to `Float64` as needed by the visualizer.

Here is how to create a nested `StructArray`:

snippet: howto/component_mapping[nested_struct]

The following remaps the `Scalars:scalars` input to read from `custom:my_nested_scalar` and selects the `values` field:

snippet: howto/component_mapping[selector_mapping]

## Providing default values

You can also force a visualizer to use a specific source kind. Setting the source to `Default` makes the visualizer
ignore any store data and use the view's default instead:

snippet: howto/component_mapping[custom_value]

## Full example

The complete example logs three series to a single entity and configures each with a different component mapping strategy. This leads to the following visualizers for the `/plot` entity:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mapping-all-visualizers/82b4ea0a8290bb3b5043296cae810002c1bd5174/full.png" alt="Visualizer configuration for the /plot entity after remapping components">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mapping-all-visualizers/82b4ea0a8290bb3b5043296cae810002c1bd5174/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mapping-all-visualizers/82b4ea0a8290bb3b5043296cae810002c1bd5174/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mapping-all-visualizers/82b4ea0a8290bb3b5043296cae810002c1bd5174/1024w.png">
</picture>

* 🐍 [Python](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/howto/component_mapping.py)
* 🦀 [Rust](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/howto/component_mapping.rs)

<picture>
  <img src="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/full.png" alt="Three series plotted from a single entity using different component mapping strategies">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/component-mapping-result/93fb68e03744e5ace721e2a8b49eebf5c39d9076/1200w.png">
</picture>

When the view is selected, the selection panel shows an overview of all configured visualizers:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/component-mapping-visualizer-list/48e05924eac3a4a93847299707132c73fe10609c/full.png" alt="Visualizer list in the selection panel showing the three configured series">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/component-mapping-visualizer-list/48e05924eac3a4a93847299707132c73fe10609c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/component-mapping-visualizer-list/48e05924eac3a4a93847299707132c73fe10609c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/component-mapping-visualizer-list/48e05924eac3a4a93847299707132c73fe10609c/1024w.png">
</picture>
