<!--[metadata]
title = "ROS bridge"
source = "https://github.com/rerun-io/cpp-example-ros-bridge"
tags = ["2D", "3D", "Mesh", "Pinhole camera", "ROS", "Time series", "C++"]
thumbnail = "https://static.rerun.io/ros_bridge/121f72ebaea57a1b895196a5587fd1a428a9fd0e/480w.png"
thumbnail_dimensions = [480, 480]
-->

A proof-of-concept Rerun bridge for ROS 1 that subscribes to all supported topics and visualizes the messages in Rerun.

## Background

This is an example that shows how to use Rerun's C++ API to log and visualize [ROS](https://www.ros.org/) messages.

It works by subscribing to all topics with supported types, converting the messages, and logging the data to Rerun. It further allows to remap topic names to specific entity paths, specify additional static transforms, and pinhole parameters via an external config file. See the [launch](https://github.com/rerun-io/cpp-example-ros-bridge/tree/main/rerun_bridge/launch) directory for usage examples.


<picture>
  <img src="https://static.rerun.io/ros_bridge_screenshot/42bcbe797ff18079678b08a6ee0551fcdb7f054b/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/ros_bridge_screenshot/42bcbe797ff18079678b08a6ee0551fcdb7f054b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ros_bridge_screenshot/42bcbe797ff18079678b08a6ee0551fcdb7f054b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/ros_bridge_screenshot/42bcbe797ff18079678b08a6ee0551fcdb7f054b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ros_bridge_screenshot/42bcbe797ff18079678b08a6ee0551fcdb7f054b/1200w.png">
</picture>

## Run the code

This is an external example, check the [repository](https://github.com/rerun-io/cpp-example-ros-bridge) for more information.

In a nutshell, clone the repo and run a demo with:

```
pixi run {spot_,drone_}example
```

Note that this example currently supports Linux only.
