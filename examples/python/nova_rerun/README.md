<!--[metadata]
title = "Nova Bridge"
tags = ["3D", "Robot"]
thumbnail = "https://static.rerun.io/nova-bridge/xyz123/480w.png"
thumbnail_dimensions = [480, 480]
-->

[Wandelbots Nova](https://www.wandelbots.com/) is a robot-agnostic operating system that enables programming and controlling various industrial robots through a unified interface. This example demonstrates how to use Nova and Rerun to visualize robot trajectories and real-time states for any supported industrial robot.

https://github.com/user-attachments/assets/4b18a6b6-b946-45af-9ade-614ca9d321a6

## Used Rerun types

[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d), [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d), [`Points3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d)

## Background

[Wandelbots Nova](https://www.wandelbots.com/) is a platform for robot programming and control that provides a unified interface for industrial six-axis robots across different manufacturers. It combines modern development tools (Python, JavaScript APIs) with an AI-driven approach to robot control and motion planning.

The platform enables developers to program industrial applications like gluing, grinding, welding, and palletizing through a consistent API, regardless of the underlying robot hardware. This example demonstrates how to use Rerun to visualize and analyze these capabilities through:

-   Trajectory visualization and motion planning
-   Robot state monitoring and digital twin visualization
-   Collision scene inspection, avoidance and validation
-   Motion timing and performance analysis

## Logging and visualizing with Rerun

### Setting up the bridge

To use the bridge you need to install the [wandelbots-nova](https://github.com/wandelbotsgmbh/wandelbots-nova) package and apply for a instance and access token at [wandelbots.com](https://www.wandelbots.com/).

```bash
poetry install wandelbots-nova --extras "nova-rerun-bridge"

# Download the required models
poetry run download-models
```

The example creates a bridge between Nova and rerun:

```python
from nova_rerun_bridge import NovaRerunBridge
from nova import Nova

nova = Nova(
    host="https://your-instance.wandelbots.io",
    access_token="your-access-token"
)
bridge = NovaRerunBridge(nova)

# Setup default visualization blueprint
await bridge.setup_blueprint()
```

### Collision free movements

Apart from the usual movement commands like `point to point`, `joint point to point`, `linear` and `circular` the plattform also supports collision free movements. You need to setup a collision scene beforehand and pass it to the action.

```python
actions = [
    collision_free(
        target=Pose((-500, -400, 200, np.pi, 0, 0)),
        collision_scene=collision_scene,
        settings=MotionSettings(tcp_velocity_limit=30),
    )
]

trajectory_plan_combined = await motion_group.plan(
    actions,
    tcp=tcp,
    start_joint_position=joint_trajectory.joint_positions[-1].joints,
)
await bridge.log_actions(welding_actions)
await bridge.log_trajectory(trajectory_plan_combined, tcp, motion_group)
```

https://github.com/user-attachments/assets/6372e614-80c1-4804-bac7-b9b8b29da533

### Logging robot trajectories

Once configured, you can easily log planned trajectories:

```python
# Plan and log a trajectory
joint_trajectory = await motion_group.plan(actions, tcp)
await bridge.log_trajectory(joint_trajectory, tcp, motion_group)
```

### Real-time robot state streaming

The bridge also supports continuous monitoring of robot states:

```python
# Start streaming robot state
await bridge.start_streaming(motion_group)

# Stop streaming all robot states
await bridge.stop_streaming()
```
