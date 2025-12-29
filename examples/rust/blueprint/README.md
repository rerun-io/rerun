# Blueprint Example

This example demonstrates how to use the Rerun blueprint API to programmatically configure the viewer layout.

## What it shows

- Creating a blueprint with a `Horizontal` container
- Adding multiple `TimeSeriesView` instances
- Controlling automatic view creation with `with_auto_views()`
- Sending the blueprint to the viewer

## Running the example

```bash
# Run with the blueprint (auto_views disabled by default)
cargo run -p blueprint

# Run with automatic view creation enabled
cargo run -p blueprint -- --auto-views
```

## Code Overview

The example creates a horizontal layout with two time series views, then logs some sine and cosine data. The blueprint configuration ensures the viewer displays the data in the specified layout instead of using automatic heuristics.

This is the Rust equivalent of the Python `examples/python/blueprint/blueprint.py` example.
