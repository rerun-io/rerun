<!--[metadata]
title = "MCAP"
tags = ["MCAP", "RRD", "ROS", "ROS 2", "Rosbag", "Tutorial"]
source = "https://github.com/rerun-io/mcap_example"
thumbnail = "https://static.rerun.io/mcap_example/7a3207652fa411979a96d5c5a25a43be29f1fdfb/480w.png"
thumbnail_dimensions = [480, 305]
-->

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/d69eb734367556854dcb167f8b8f8f9f6d3760e0_mcap_example.mp4" type="video/mp4" />
</video>

## Background

This example demonstrates how to visualize and work with [MCAP](https://mcap.dev/) files in Rerun. From [mcap.dev](https://mcap.dev/):

> MCAP (pronounced "em-cap") is an open source container file format for multimodal log data. It supports multiple channels of timestamped pre-serialized data, and is ideal for use in pub/sub or robotics applications.

MCAP is the default bag format in ROS 2 and is rapidly gaining adoption. You can read more about [Rerun's MCAP support here](https://rerun.io/docs/howto/mcap).

In this guide, you will learn:

1. How to **load MCAP files** directly into the Rerun viewer.
2. How to **convert MCAP files** into native Rerun data files (**RRD**).
3. How to **convert older ROS bags** (ROS 1 and ROS 2 SQLite3) into MCAP.
4. How to read and deserialize MCAP/RRD data in Python for programmatic processing and advanced visualization in Rerun.

We will use a dataset from the [JKK Research Center](https://jkk-research.github.io/dataset/jkk_dataset_01/) containing LiDAR, images, GPS, and IMU data. The workflow involves converting the original ROS 1 bag → MCAP → RRD, and then using Python to log the RRD data with specific Rerun components for optimal visualization.

## Follow the tutorial and run the code

This is an external example. Check the [mcap_example](https://github.com/rerun-io/mcap_example) repository for more information.
