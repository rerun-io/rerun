# Overview

A minimal example of creating a ROS node that subscribes to topics and converts the messages to rerun log calls.

The solution here is mostly a toy example to show how ROS concepts can be mapped to Rerun. Fore more information on
future improved ROS support, see the tracking issue: [#1527](https://github.com/rerun-io/rerun/issues/1537)

NOTE: Unlike many of the other examples, this example requires a system installation of ROS
in addition to the packages from requirements.txt.

# Dependencies

This example was developed and tested on top of [ROS2 Humble Hawksbill](https://docs.ros.org/en/humble/index.html)
and the the [turtlebot3 navigation example](https://navigation.ros.org/getting_started/index.html).

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


# Running

First, in one terminal launch the nav2 turtlebot demo:
```
source /opt/ros/humble/setup.bash
export TURTLEBOT3_MODEL=waffle
export GAZEBO_MODEL_PATH=$GAZEBO_MODEL_PATH:/opt/ros/humble/share/turtlebot3_gazebo/models

ros2 launch nav2_bringup tb3_simulation_launch.py headless:=False
```

As described in the nav demo, use the rviz window to initialize the pose estimate and set a navigation goal.

You can now connect to the running ROS system by running:
```
python3 examples/python/ros/main.py
```


