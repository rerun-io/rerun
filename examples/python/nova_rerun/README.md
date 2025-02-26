<!--[metadata]
title = "Nova Bridge"
source = "https://github.com/wandelbotsgmbh/wandelbots-nova"
tags = ["3D", "Robot"]
thumbnail = "https://github.com/user-attachments/assets/526a3cff-3d27-4963-8d7b-e7cd98a758f9"
thumbnail_dimensions = [480, 480]
-->

A visualization extension for [wandelbots-nova](https://github.com/wandelbotsgmbh/wandelbots-nova) that enables real-time 3D visualization of robot trajectories using [rerun.io](https://rerun.io).

https://github.com/user-attachments/assets/4b18a6b6-b946-45af-9ade-614ca9d321a6

## Background

[Wandelbots Nova](https://www.wandelbots.com/) is a robot-agnostic platform for programming and controlling industrial six-axis robots across different manufacturers through a unified API. It combines modern development tools (Python, JavaScript APIs) with an AI-driven approach to robot control and motion planning, enabling developers to build applications like gluing, grinding, welding, and palletizing without worrying about underlying hardware differences.

This example demonstrates how to use Rerun to visualize and analyze Nova’s capabilities through:

-   Trajectory visualization and motion planning
-   Robot state monitoring and digital twin visualization
-   Collision scene inspection, avoidance and validation
-   Motion timing and performance analysis

### Run the code

To use the bridge you need to install the [wandelbots-nova](https://github.com/wandelbotsgmbh/wandelbots-nova) package and apply for a instance and access token at [wandelbots.com](https://www.wandelbots.com/).

```bash
uv run download-models # Download the required models
uv run main.py
```

The example demonstrates how to use Wandelbots Nova with Rerun for visualizing planned robot trajectories.

```python
from nova_rerun_bridge import NovaRerunBridge
from nova import Nova
from nova import api
from nova.actions import jnt, ptp
from nova.types import Pose
import asyncio

async def main():
  # Connect to your Nova instance (or use .env file)
  nova = Nova(
      host="https://your-instance.wandelbots.io",
      access_token="your-access-token"
  )
  bridge = NovaRerunBridge(nova)

  # Setup visualization
  await bridge.setup_blueprint()

  # Setup robot
  cell = nova.cell()
  controller = await cell.ensure_virtual_robot_controller(
      "ur",
      api.models.VirtualControllerTypes.UNIVERSALROBOTS_MINUS_UR10E,
      api.models.Manufacturer.UNIVERSALROBOTS,
  )

  # Connect to the controller and activate motion groups
  async with controller[0] as motion_group:
      home_joints = await motion_group.joints()
      tcp_names = await motion_group.tcp_names()
      tcp = tcp_names[0]

      # Get current TCP pose and offset it slightly along the x-axis
      current_pose = await motion_group.tcp_pose(tcp)
      target_pose = current_pose @ Pose((1, 0, 0, 0, 0, 0))

      actions = [
          jnt(home_joints),
          ptp(target_pose),
          jnt(home_joints),
      ]

      # Plan trajectory
      joint_trajectory = await motion_group.plan(actions, tcp)

      # Log a trajectory
      await bridge.log_trajectory(joint_trajectory, tcp, motion_group)


if __name__ == "__main__":
    asyncio.run(main())
```

### Features

-   **Real-time 3D robot visualization**
    See a [list of supported robots](https://wandelbotsgmbh.github.io/wandelbots-js-react-components/?path=/story/3d-view-robot-supported-models--abb-1010-037-15) and get real-time playback of robot movements.
-   **Trajectory playback and analysis**
    Easily log trajectories and visualize them in the Rerun viewer.
-   **Collision scene visualization**
    Inspect collision objects, plan safe paths, and avoid unexpected contact points.
-   **Continuous monitoring mode**
    Stream live robot states and keep an eye on actual motion in real time.

<img width="100%" src="https://github.com/user-attachments/assets/b42b7ac8-f142-4b65-9fd8-22b020ce753e" />

### Usage Examples

Below are some common usage patterns. For more detailed examples, see the [example repository.](https://github.com/wandelbotsgmbh/wandelbots-nova/tree/main/nova_rerun_bridge/examples)

#### Basic Motion Logging

```python
# Log a pre-planned trajectory
await bridge.log_trajectory(joint_trajectory, tcp, motion_group)
```

#### Collision free movements

Apart from the usual movement commands like `point to point`, `joint point to point`, `linear` and `circular` the plattform also supports collision free movements. You need to setup a collision scene beforehand and pass it to the action.

```python
actions = [
    collision_free(
        target=Pose((-500, -400, 200, np.pi, 0, 0)),
        collision_scene=collision_scene,
        settings=MotionSettings(tcp_velocity_limit=30),
    )
]

trajectory = await motion_group.plan(
    actions,
    tcp=tcp
)
await bridge.log_actions(actions)
await bridge.log_trajectory(trajectory_plan_combined, tcp, motion_group)
```

https://github.com/user-attachments/assets/6372e614-80c1-4804-bac7-b9b8b29da533

#### Real-time robot state streaming

The bridge also supports continuous monitoring of robot states:

```python
# Start streaming robot state
await bridge.start_streaming(motion_group)

# ... do something with your robot ...

# Stop streaming all robot states
await bridge.stop_streaming()
```

#### Log Actions

If you’d like to log the planned actions themselves:

```python
# Log planned actions
await bridge.log_actions(actions)
```

### More Examples

Check out the [examples folder](https://github.com/wandelbotsgmbh/wandelbots-nova/tree/main/nova_rerun_bridge/examples) for more detailed usage scenarios (e.g., advanced collision scene management, streaming multiple robots simultaneously, etc.).
