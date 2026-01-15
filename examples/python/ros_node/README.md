<!--[metadata]
title = "ROS node"
tags = ["2D", "3D", "Pinhole camera", "ROS", "Time series", "URDF"]
thumbnail = "https://static.rerun.io/ros_node_example/ddc3387995cda1b283a5c58ffbc6021d91abde7d/480w.png"
thumbnail_dimensions = [480, 284]
-->

A minimal example of creating a ROS node that subscribes to topics and converts the messages to Rerun log calls.

The solution here is mostly a toy example to show how ROS concepts can be mapped to Rerun.

<picture>
  <img src="https://static.rerun.io/ros_node_example/ddc3387995cda1b283a5c58ffbc6021d91abde7d/full.png" alt="Rerun viewer showing data streamed from the example ROS node">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/ros_node_example/ddc3387995cda1b283a5c58ffbc6021d91abde7d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ros_node_example/ddc3387995cda1b283a5c58ffbc6021d91abde7d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/ros_node_example/ddc3387995cda1b283a5c58ffbc6021d91abde7d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ros_node_example/ddc3387995cda1b283a5c58ffbc6021d91abde7d/1200w.png">
</picture>

## Used Rerun types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`DepthImage`](https://rerun.io/docs/reference/types/archetypes/depth_image), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`LineStrips3D`](https://www.rerun.io/docs/reference/types/archetypes/line_strips3d), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars)

## Background
The [Robot Operating System (ROS)](https://www.ros.org) helps build robot applications through software libraries and tools.
Although Rerun doesn't have native ROS support, you can easily create a basic ROS 2 Python node to subscribe to common ROS topics and log them to Rerun.
In this example, Rerun visualizes simulation data, including robot pose, images, camera position, laser scans, point clouds, and velocities, as the robot navigates the environment.

## Logging and visualizing with Rerun

Find the detailed code walkthrough and explanation for visualizing this example here: [Using Rerun with ROS 2](https://www.rerun.io/docs/howto/ros2-nav-turtlebot).

For more information on future improved ROS support, see tracking issue: [#1527](https://github.com/rerun-io/rerun/issues/1537)

## Run the code

### Dependencies

> NOTE: Unlike many of the other examples, this example requires a system installation of ROS
in addition to the packages from requirements.txt.

This example was developed and tested on top of [ROS2 Kilted Kaiju](https://docs.ros.org/en/kilted/index.html), with [Nav2](https://docs.nav2.org/) and the Turtlebot 4 simulation example from the [nav2_bringup](https://github.com/ros-navigation/navigation2/tree/main/nav2_bringup) package.

Installing ROS is outside the scope of this example, but you will need the equivalent of the following packages:
```
sudo apt install ros-kilted-desktop ros-kilted-nav2-bringup
```
(`ros-kilted-nav2-bringup` pulls in all the navigation and simulation packages we need as dependencies, if not installed yet)

Then clone the Rerun repository to get the example code:
```bash
git clone https://github.com/rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```

Make sure to use a Python virtual environment. Here, we use `venv` (`sudo apt install python3-venv`):
```bash
python3 -m venv --system-site-packages rerun-ros-example
source rerun-ros-example/bin/activate
```

Then install the latest Rerun SDK and the necessary libraries specified in the requirements file of this example:
```bash
pip install --upgrade rerun-sdk
pip install -r examples/python/ros_node/requirements.txt
```

In addition to installing the dependencies from `requirements.txt` into a venv you will also need to source the
ROS setup script:
```bash
source /opt/ros/kilted/setup.bash
```

### Run the code

First, in one terminal launch the Nav2 turtlebot demo:
```bash
source /opt/ros/kilted/setup.bash

ros2 launch nav2_bringup tb4_simulation_launch.py headless:=False
```

This should open two windows for Gazebo and RViz. Use the RViz window to initialize the pose estimate to put the robot on the map, and set a navigation goal to let it move.

You can now connect to the running ROS system by running this in a separate terminal:
```bash
source /opt/ros/kilted/setup.bash

python examples/python/ros_node/main.py # run the example
```

If you wish to customize it, or explore additional features, use the CLI with the `--help` option for guidance:
```bash
python examples/python/ros_node/main.py --help
```
