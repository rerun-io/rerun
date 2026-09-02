---
title: "MCAP importer maps ROS scalar sensor messages to `Measurements`"
hidden: true
type: breaking
---

### MCAP importer maps ROS scalar sensor messages to `Measurements`

`sensor_msgs/msg/Temperature`, `FluidPressure`, `Illuminance`, and `RelativeHumidity` now import as the [`Measurements`](../reference/types/archetypes/measurements.md) archetype instead of `Scalars` plus `SeriesLines`.
The `variance` field of the message becomes the uncertainty of the measurement, drawn as a one-sigma band around the line, and the unit of the message (`°C`, `Pa`, `lux`) shows up in the legend and in tooltips.
Previously the variance was plotted as a second series next to the value.

This does not impact schema stability for registration on an existing catalog dataset.
Existing recordings keep working as they are, but queries against newly imported data need the new component columns:

```
# Before, one column holding [value, variance] per row, plus a static series name column
Scalars:scalars
SeriesLines:names

# After, one column per quantity
Measurements:values
Measurements:variances
Measurements:units
```

`RelativeHumidity` emits no unit, since the value is a ratio in `[0, 1]`.
