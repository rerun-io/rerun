#!/usr/bin/env python3
"""A test series for view coordinates."""
from __future__ import annotations

import argparse

import numpy as np
import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk

parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
rr.script_add_args(parser)
args = parser.parse_args()

rr.script_setup(args, "rerun_example_view_coordinates")

# Log sphere of colored points to make it easier to orient ourselves.
# See https://math.stackexchange.com/a/1586185
num_points = 5000
radius = 8
lamd = np.arccos(2 * np.random.rand(num_points) - 1) - np.pi / 2
phi = np.random.rand(num_points) * 2 * np.pi
x = np.cos(lamd) * np.cos(phi)
y = np.cos(lamd) * np.sin(phi)
z = np.sin(lamd)
unit_sphere_positions = np.transpose([x, y, z])
rr.log("world/points", rr.Points3D(unit_sphere_positions * radius, colors=np.abs(unit_sphere_positions), radii=0.01))

# RGB image that indicates orientation:
rgb = np.zeros((50, 100, 3))
rgb[0:3, 0:3] = [255, 255, 255]
rgb[3:25, 0:3] = [0, 255, 0]
rgb[0:3, 3:25] = [255, 0, 0]

# Depth image for testing depth cloud:
# depth = np.ones((50, 100)) * 0.5
x, y = np.meshgrid(np.arange(0, 100), np.arange(0, 50))
depth = 0.5 + 0.005 * x + 0.25 * np.sin(3.14 * y / 50 / 2)


rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP)


def log_camera(origin: npt.ArrayLike, label: str, xyz: rr.components.ViewCoordinates, forward: npt.ArrayLike) -> None:
    [height, width, _channels] = rgb.shape
    f_len = (height * width) ** 0.5
    cam_path = f"world/{label}"
    pinhole_path = f"{cam_path}/{label}"
    rr.log(f"{cam_path}/indicator", rr.Points3D([0, 0, 0], colors=[255, 255, 255], labels=label))
    rr.log(cam_path, rr.Transform3D(translation=origin))
    rr.log(cam_path + "/arrow", rr.Arrows3D(origins=[0, 0, 0], vectors=forward, colors=[255, 255, 255], radii=0.025))
    rr.log(
        pinhole_path,
        rr.Pinhole(
            width=width,
            height=height,
            focal_length=f_len,
            principal_point=[width * 3 / 4, height * 3 / 4],  # test offset principal point
            camera_xyz=xyz,
        ),
    )
    rr.log(f"{pinhole_path}/rgb", rr.Image(rgb))
    rr.log(f"{pinhole_path}/depth", rr.DepthImage(depth, meter=1.0))


# Log a series of pinhole cameras only differing by their view coordinates and some offset.
# Not all possible, but a fair sampling.

s = 3  # spacing

log_camera([0, 0, s], "RUB", rr.ViewCoordinates.RUB, forward=[0, 0, -1])

# All right-handed permutations of RDF:
log_camera([s, -s, 0], "RDF", rr.ViewCoordinates.RDF, forward=[0, 0, 1])
log_camera([s, 0, 0], "FRD", rr.ViewCoordinates.FRD, forward=[1, 0, 0])
log_camera([s, s, 0], "DFR", rr.ViewCoordinates.DFR, forward=[0, 1, 0])

# All right-handed permutations of LUB:
log_camera([0, -s, 0], "ULB", rr.ViewCoordinates.ULB, forward=[0, 0, -1])
log_camera([0, 0, 0], "LBU", rr.ViewCoordinates.LBU, forward=[0, -1, 0])
log_camera([0, s, 0], "BUL", rr.ViewCoordinates.BUL, forward=[-1, 0, 0])

# All permutations of LUF:
log_camera([-s, -s, 0], "LUF", rr.ViewCoordinates.LUF, forward=[0, 0, 1])
log_camera([-s, 0, 0], "FLU", rr.ViewCoordinates.FLU, forward=[1, 0, 0])
log_camera([-s, s, 0], "UFL", rr.ViewCoordinates.UFL, forward=[0, 1, 0])


rr.script_teardown(args)
