<!--[metadata]
title = "ROS TF"
tags = ["ROS", "TF", "Transform", "Coordinate Frame", "ROS 2"]
source = "https://github.com/rerun-io/ros_tf_example"
thumbnail = "https://static.rerun.io/ros_tf_example/6fd0961787faa8ed6428769d7cbe37d136915535/480w.png"
thumbnail_dimensions = [480, 305]
-->

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/749da3520d9d6bd2b8f5f73d878f4428780f5130_ros_tf_example.mp4" type="video/mp4" />
</video>

## Background

ROS 2 uses the transform library, [tf2](https://docs.ros.org/en/jazzy/Concepts/Intermediate/About-Tf2.html), to track multiple coordinate frames over time. It is a powerful system that allows developers to transform points, vectors, and poses between different frames of reference (e.g., from a "camera_link" to "base_link"). This system makes collaboration between developers around the world easier, as it provides a common language for how transforms should be handled — a topic that can otherwise be defined in many different ways. In Rerun, you can use [named transforms](https://rerun.io/docs/concepts/logging-and-ingestion/transforms#named-transform-frames) to decouple spatial relationships from the entity hierarchy, similar to as it is done in ROS.

The Rerun documentation already contain guides for how to work with named transforms and how to turn your ROS 2 transforms into Rerun transforms (see section *useful resources* below). Instead of repeating the documentation, this example will show you how to debug your system when transforms do not work. For this, we will use the [JKK Research Center](https://jkk-research.github.io/dataset/jkk_dataset_01/) dataset. You will see that when simply dropping the dataset into the Rerun viewer, it will complain about missing transforms and broken transform paths. We will go through how to debug and fix these problems.

## Follow the tutorial and run the code

This is an external example. Check the [ros_tf_example](https://github.com/rerun-io/ros_tf_example) repository for more information.
