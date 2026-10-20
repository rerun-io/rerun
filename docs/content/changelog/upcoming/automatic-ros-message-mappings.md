---
title: Automatic visualization of raw ROS messages
hidden: true
type: feature
---

### Automatic visualization of raw ROS messages

Previously, you could already visualize ROS messages without converting them to Rerun's semantic types at ingest time by configuring [component mappings](../howto/visualization/component-mappings.md) in a blueprint.
However, this can be a tedious process!

The Viewer's heuristic blueprint is now aware of various ROS types and configures these runtime mappings automatically where supported.

For example, `std_msgs/msg/String` appears as text, while `sensor_msgs/msg/Temperature` and `sensor_msgs/msg/FluidPressure` appear as a [`Measurements`](../reference/types/archetypes/measurements.md) archetype with units and variance bands.

Currently supported: `std_msgs/msg/String`, `sensor_msgs/msg/Temperature`, `sensor_msgs/msg/FluidPressure`, `sensor_msgs/msg/RelativeHumidity`, `sensor_msgs/msg/Illuminance`, and `sensor_msgs/msg/MagneticField`.
Other ROS 2 messages still need import-time conversion to be visualized automatically.
See [Supported Message Formats](../concepts/logging-and-ingestion/mcap/message-formats.md#visualizing-ros-2-messages-without-conversion) for details.

<picture>
  <img src="https://static.rerun.io/heuristic-lensing/f688680e5a84910692f11448f309a310e8085497/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/heuristic-lensing/f688680e5a84910692f11448f309a310e8085497/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/heuristic-lensing/f688680e5a84910692f11448f309a310e8085497/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/heuristic-lensing/f688680e5a84910692f11448f309a310e8085497/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/heuristic-lensing/f688680e5a84910692f11448f309a310e8085497/1200w.png">
</picture>
