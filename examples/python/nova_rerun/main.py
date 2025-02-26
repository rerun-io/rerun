"""
Example of planning a collision free PTP motion. A sphere is placed in the robot's path and the robot uses collision free p2p to move around it.
"""

import asyncio

import numpy as np
import rerun as rr
from nova import MotionSettings
from nova.actions.motions import collision_free, ptp
from nova.api import models
from nova.core.exceptions import PlanTrajectoryFailed
from nova.core.nova import Nova
from nova.types import Pose
from nova_rerun_bridge import NovaRerunBridge
from wandelbots_api_client.models import (
    CoordinateSystem,
    RotationAngles,
    RotationAngleTypes,
    Vector3d,
)


async def build_collision_world(
    nova: Nova, cell_name: str, robot_setup: models.OptimizerSetup
) -> str:
    collision_api = nova._api_client.store_collision_components_api
    scene_api = nova._api_client.store_collision_scenes_api

    # define annoying obstacle
    sphere_collider = models.Collider(
        shape=models.ColliderShape(models.Sphere2(radius=100, shape_type="sphere")),
        pose=models.Pose2(position=[-100, -500, 200]),
    )
    await collision_api.store_collider(
        cell=cell_name, collider="annoying_obstacle", collider2=sphere_collider
    )

    # define TCP collider geometry
    tool_collider = models.Collider(
        shape=models.ColliderShape(
            models.Box2(size_x=100, size_y=100, size_z=100, shape_type="box", box_type="FULL")
        )
    )
    await collision_api.store_collision_tool(
        cell=cell_name, tool="tool_box", request_body={"tool_collider": tool_collider}
    )

    # define robot link geometries
    robot_link_colliders = await collision_api.get_default_link_chain(
        cell=cell_name, motion_group_model=robot_setup.motion_group_type
    )
    await collision_api.store_collision_link_chain(
        cell=cell_name, link_chain="robot_links", collider=robot_link_colliders
    )

    # assemble scene
    scene = models.CollisionScene(
        colliders={"annoying_obstacle": sphere_collider},
        motion_groups={
            robot_setup.motion_group_type: models.CollisionMotionGroup(
                tool={"tool_geometry": tool_collider}, link_chain=robot_link_colliders
            )
        },
    )
    scene_id = "collision_scene"
    await scene_api.store_collision_scene(
        cell_name, scene_id, models.CollisionSceneAssembly(scene=scene)
    )
    return scene_id


async def test():
    async with Nova() as nova, NovaRerunBridge(nova) as bridge:
        await bridge.setup_blueprint()

        cell = nova.cell()
        controller = await cell.ensure_virtual_robot_controller(
            "ur5",
            models.VirtualControllerTypes.UNIVERSALROBOTS_MINUS_UR5E,
            models.Manufacturer.UNIVERSALROBOTS,
        )

        await nova._api_client.virtual_robot_setup_api.set_virtual_robot_mounting(
            cell="cell",
            controller=controller.controller_id,
            id=0,
            coordinate_system=CoordinateSystem(
                coordinate_system="world",
                name="mounting",
                reference_uid="",
                position=Vector3d(x=0, y=0, z=0),
                rotation=RotationAngles(
                    angles=[0, 0, 0], type=RotationAngleTypes.EULER_ANGLES_EXTRINSIC_XYZ
                ),
            ),
        )

        # NC-1047
        await asyncio.sleep(5)

        # Connect to the controller and activate motion groups
        async with controller[0] as motion_group:
            await bridge.log_saftey_zones(motion_group)

            tcp = "Flange"

            robot_setup: models.OptimizerSetup = await motion_group._get_optimizer_setup(tcp=tcp)
            robot_setup.safety_setup.global_limits.tcp_velocity_limit = 200

            collision_scene_id = await build_collision_world(nova, "cell", robot_setup)

            await bridge.log_collision_scenes()

            # Use default planner to move to the right of the sphere
            home = await motion_group.tcp_pose(tcp)
            actions = [ptp(home), ptp(target=Pose((300, -400, 200, np.pi, 0, 0)))]

            for action in actions:
                action.settings = MotionSettings(tcp_velocity_limit=200)

            try:
                joint_trajectory = await motion_group.plan(
                    actions, tcp, start_joint_position=(0, -np.pi / 2, np.pi / 2, 0, 0, 0)
                )
                await bridge.log_actions(actions)
                await bridge.log_trajectory(joint_trajectory, tcp, motion_group)
            except PlanTrajectoryFailed as e:
                await bridge.log_actions(actions)
                await bridge.log_trajectory(e.error.joint_trajectory, tcp, motion_group)
                await bridge.log_error_feedback(e.error.error_feedback)

            rr.log(
                "motion/target_", rr.Points3D([[-500, -400, 200]], radii=[10], colors=[(0, 255, 0)])
            )

            # Use default planner to move to the left of the sphere
            # -> this will collide
            # only plan don't move
            actions = [ptp(target=Pose((-500, -400, 200, np.pi, 0, 0)))]

            for action in actions:
                action.settings = MotionSettings(tcp_velocity_limit=200)

            try:
                joint_trajectory_with_collision = await motion_group.plan(
                    actions, tcp, start_joint_position=joint_trajectory.joint_positions[-1].joints
                )
                await bridge.log_actions(actions)
                await bridge.log_trajectory(joint_trajectory_with_collision, tcp, motion_group)
            except PlanTrajectoryFailed as e:
                await bridge.log_actions(actions)
                await bridge.log_trajectory(e.error.joint_trajectory, tcp, motion_group)
                await bridge.log_error_feedback(e.error.error_feedback)

            # Plan collision free PTP motion around the sphere
            scene_api = nova._api_client.store_collision_scenes_api
            collision_scene = await scene_api.get_stored_collision_scene(
                cell="cell", scene=collision_scene_id
            )

            welding_actions = [
                collision_free(
                    target=Pose((-500, -400, 200, np.pi, 0, 0)),
                    collision_scene=collision_scene,
                    settings=MotionSettings(tcp_velocity_limit=30),
                )
            ]

            trajectory_plan_combined = await motion_group.plan(
                welding_actions,
                tcp=tcp,
                start_joint_position=joint_trajectory.joint_positions[-1].joints,
            )
            await bridge.log_actions(welding_actions)
            await bridge.log_trajectory(trajectory_plan_combined, tcp, motion_group)


if __name__ == "__main__":
    asyncio.run(test())
