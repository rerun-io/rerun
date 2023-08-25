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

rr.rscript_setup(args, "rerun-example-view_coordinates")

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
rr.log_points(
    "world/points", positions=unit_sphere_positions * radius, colors=np.abs(unit_sphere_positions), radii=0.01
)

# RGB image that indicates orientation:
rgb = np.zeros((50, 100, 3))
rgb[0:3, 0:3] = [255, 255, 255]
rgb[3:25, 0:3] = [0, 255, 0]
rgb[0:3, 3:25] = [255, 0, 0]

# Depth image for testing depth cloud:
# depth = np.ones((50, 100)) * 0.5
x, y = np.meshgrid(np.arange(0, 100), np.arange(0, 50))
depth = 0.5 + 0.005 * x + 0.25 * np.sin(3.14 * y / 50 / 2)


rr.log_view_coordinates("world", up="+Z")


def log_camera(origin: npt.ArrayLike, xyz: str, forward: npt.ArrayLike) -> None:
    [height, width, _channels] = rgb.shape
    f_len = (height * width) ** 0.5
    cam_path = f"world/{xyz}"
    pinhole_path = f"{cam_path}/{xyz}"
    rr.log_point(f"{cam_path}/indicator", position=[0, 0, 0], color=[255, 255, 255], label=xyz)
    rr.log_transform3d(cam_path, transform=rr.Translation3D(origin))
    rr.log_arrow(cam_path + "/arrow", origin=[0, 0, 0], vector=forward, color=[255, 255, 255], width_scale=0.025)
    rr.log_pinhole(
        pinhole_path,
        width=width,
        height=height,
        focal_length_px=f_len,
        principal_point_px=[width * 3 / 4, height * 3 / 4],  # test offset principal point
        camera_xyz=xyz,
    )
    rr.log_image(f"{pinhole_path}/rgb", rgb)
    rr.log_depth_image(f"{pinhole_path}/depth", depth, meter=1.0)


# Log a series of pinhole cameras only differing by their view coordinates and some offset.
# Not all possible, but a fair sampling.

s = 3  # spacing

log_camera([0, 0, s], "RUB", forward=[0, 0, -1])

# All right-handed permutations of RDF:
log_camera([s, -s, 0], "RDF", forward=[0, 0, 1])
log_camera([s, 0, 0], "FRD", forward=[1, 0, 0])
log_camera([s, s, 0], "DFR", forward=[0, 1, 0])

# All right-handed permutations of LUB:
log_camera([0, -s, 0], "ULB", forward=[0, 0, -1])
log_camera([0, 0, 0], "LBU", forward=[0, -1, 0])
log_camera([0, s, 0], "BUL", forward=[-1, 0, 0])

# All permutations of LUF:
log_camera([-s, -s, 0], "LUF", forward=[0, 0, 1])
log_camera([-s, 0, 0], "FLU", forward=[1, 0, 0])
log_camera([-s, s, 0], "UFL", forward=[0, 1, 0])


rr.script_teardown(args)
