---
title: "Scalar"
---

Log a double-precision scalar.

The current timeline value will be used for the time/X-axis, hence scalars
cannot be timeless.

When used to produce a plot, this archetype is used to provide the data that
is referenced by the `SeriesLine` or `SeriesPoint` archetypes. You can do
this by logging both archetypes to the same path, or alternatively configuring
the plot-specific archetypes through the blueprint.

## Components

**Required**: [`Scalar`](../components/scalar.md)

## Links
 * 🌊 [C++ API docs for `Scalar`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Scalar.html?speculative-link)
 * 🐍 [Python API docs for `Scalar`](https://ref.rerun.io/docs/python/stable/common/archetypes?speculative-link#rerun.archetypes.Scalar)
 * 🦀 [Rust API docs for `Scalar`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Scalar.html?speculative-link)

## Examples

### Simple line plot

code-example: scalar_simple

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/scalar_simple/8bcc92f56268739f8cd24d60d1fe72a655f62a46/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/scalar_simple/8bcc92f56268739f8cd24d60d1fe72a655f62a46/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/scalar_simple/8bcc92f56268739f8cd24d60d1fe72a655f62a46/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/scalar_simple/8bcc92f56268739f8cd24d60d1fe72a655f62a46/1200w.png">
  <img src="https://static.rerun.io/scalar_simple/8bcc92f56268739f8cd24d60d1fe72a655f62a46/full.png" width="640">
</picture>
</center>

### Multiple time series plots

code-example: scalar_multiple_plots

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/scalar_multiple/15845c2a348f875248fbd694e03eabd922741c4c/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/scalar_multiple/15845c2a348f875248fbd694e03eabd922741c4c/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/scalar_multiple/15845c2a348f875248fbd694e03eabd922741c4c/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/scalar_multiple/15845c2a348f875248fbd694e03eabd922741c4c/1200w.png">
  <img src="https://static.rerun.io/scalar_multiple/15845c2a348f875248fbd694e03eabd922741c4c/full.png" width="640">
</picture>
</center>

