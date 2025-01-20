<!--[metadata]
title = "ROS 2 bridge"
source = "https://github.com/rerun-io/cpp-example-ros2-bridge"
tags = ["2D", "3D", "Pinhole camera", "ROS", "Time series", "C++"]
thumbnail = "https://static.rerun.io/carla_thumbnail/8ec07c28f8eb901b8246afdd0b6d2b97ff75fb8d/480w.png"
thumbnail_dimensions = [480, 480]
-->

A proof-of-concept Rerun bridge for ROS 2 that subscribes to all supported topics and visualizes the messages in Rerun.

https://vimeo.com/940929187?autoplay=1&loop=1&autopause=0&background=1&muted=1&ratio=2696:1552

## Background

This is an example that shows how to use Rerun's C++ API to log and visualize [ROS 2](https://www.ros.org/) messages.

It works by subscribing to all topics with supported types, converting the messages, and logging the data to Rerun. It further allows to remap topic names to specific entity paths, specify additional static transforms, and pinhole parameters via an external config file. See the [launch](https://github.com/rerun-io/cpp-example-ros2-bridge/tree/main/rerun_bridge/launch) directory for usage examples.

## Run the code

This is an external example, check the [repository](https://github.com/rerun-io/cpp-example-ros2-bridge) for more information.

In a nutshell, clone the repo and run a demo with:

```
pixi run carla_example
```
