<!--[metadata]
title = "ROS node"
tags = ["2D", "3D", "Pinhole camera", "ROS", "Time series", "URDF"]
thumbnail = "https://static.rerun.io/ros-node/93169b35c17f5ec02d94150efb74c7ba06372842/480w.png"
thumbnail_dimensions = [480, 480]
-->

A minimal example of creating a ROS node that subscribes to topics and converts the messages to Rerun log calls.

The solution here is mostly a toy example to show how ROS concepts can be mapped to Rerun.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/1200w.png">
  <img src="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/full.png" alt="">
</picture>

## Used Rerun types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`LineStrips3D`](https://www.rerun.io/docs/reference/types/archetypes/line_strips3d), [`Scalars`](https://www.rerun.io/docs/reference/types/archetypes/scalars)

## Background
The [Robot Operating System (ROS)](https://www.ros.org) helps build robot applications through software libraries and tools.
Although Rerun doesn't have native ROS support, you can easily create a basic ROS 2 Python node to subscribe to common ROS topics and log them to Rerun.
In this example, Rerun visualizes simulation data, including robot pose, images, camera position, laser scans, point clouds, and velocities, as the [Turtlebot](http://wiki.ros.org/turtlebot3) navigates the environment.

## Logging and visualizing with Rerun

Find the detailed code walkthrough and explanation for visualizing this example here: [Using Rerun with ROS 2](https://www.rerun.io/docs/howto/ros2-nav-turtlebot).

For more information on future improved ROS support, see tracking issue: [#1527](https://github.com/rerun-io/rerun/issues/1537)

## Run the code

### Dependencies

> NOTE: Unlike many of the other examples, this example requires a system installation of ROS
in addition to the packages from requirements.txt.

This example was developed and tested on top of [ROS2 Kilted Kaiju](https://docs.ros.org/en/kilted/index.html)
and the [turtlebot3 navigation example](https://docs.nav2.org/getting_started/index.html).

Installing ROS is outside the scope of this example, but you will need the equivalent of the following packages:
```
sudo apt install ros-kilted-desktop ros-kilted-navigation2 ros-kilted-turtlebot3 ros-kilted-turtlebot3-gazebo
```

Make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/ros_node/requirements.txt
```

In addition to installing the dependencies from `requirements.txt` into a venv you will also need to source the
ROS setup script:
```
source venv/bin/active
source /opt/ros/kilted/setup.bash
```

### Run the code

First, in one terminal launch the nav2 turtlebot demo:
```
source /opt/ros/kilted/setup.bash
export TURTLEBOT3_MODEL=waffle
export GAZEBO_MODEL_PATH=$GAZEBO_MODEL_PATH:/opt/ros/kilted/share/turtlebot3_gazebo/models

ros2 launch nav2_bringup tb3_simulation_launch.py headless:=False
```

As described in the nav demo, use the rviz window to initialize the pose estimate and set a navigation goal.

You can now connect to the running ROS system by running:
```bash
python examples/python/ros_node/main.py # run the example
```

If you wish to customize it, or explore additional features, use the CLI with the `--help` option for guidance:
```bash
python examples/python/ros_node/main.py --help
```
