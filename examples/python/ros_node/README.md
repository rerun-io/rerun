<!--[metadata]
title = "ROS Node"
tags = ["2D", "3D", "mesh", "pinhole-camera", "ros", "time-series"]
thumbnail = "https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/480w.png"
thumbnail_dimensions = [480, 266]
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/1200w.png">
  <img src="https://static.rerun.io/ros_node/de224f02697d8fa26a387e497ef5823a68122356/full.png" alt="">
</picture>

A minimal example of creating a ROS node that subscribes to topics and converts the messages to rerun log calls. 
The solution here is mostly a toy example to show how ROS concepts can be mapped to Rerun. 

Fore more information on future improved ROS support, see the tracking issue: [#1527](https://github.com/rerun-io/rerun/issues/1537)

NOTE: Unlike many of the other examples, this example requires a system installation of ROS
in addition to the packages from requirements.txt.

## Used Rerun Types
[`Image`](https://www.rerun.io/docs/reference/types/archetypes/image), [`Pinhole`](https://www.rerun.io/docs/reference/types/archetypes/pinhole), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d), [`LineStrips3D`](https://www.rerun.io/docs/reference/types/archetypes/line_strips3d), [`Scalar`](https://www.rerun.io/docs/reference/types/archetypes/scalar)

# ROS Dependencies

This example was developed and tested on top of [ROS2 Humble Hawksbill](https://docs.ros.org/en/humble/index.html)
and the [turtlebot3 navigation example](https://navigation.ros.org/getting_started/index.html).

Installing ROS is outside the scope of this example, but you will need the equivalent of the following packages:
```
sudo apt install ros-humble-desktop gazebo ros-humble-navigation2 ros-humble-turtlebot3 ros-humble-turtlebot3-gazebo
```

In addition to installing the dependencies from `requirements.txt` into a venv you will also need to source the
ROS setup script:
```
source venv/bin/active
source /opt/ros/humble/setup.bash
```

# Run the Code

To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
# Setup 
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/ros_node/requirements.txt
```

In one terminal launch the nav2 turtlebot demo:
```
source /opt/ros/humble/setup.bash
export TURTLEBOT3_MODEL=waffle
export GAZEBO_MODEL_PATH=$GAZEBO_MODEL_PATH:/opt/ros/humble/share/turtlebot3_gazebo/models

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
