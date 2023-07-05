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

rr.script_setup(args, "view_coordinates")

# Log sphere of colored points to make it easier to orient ourselves.
# See https://math.stackexchange.com/a/1586185
num_points = 5000
radius = 5
lamd = np.arccos(2 * np.random.rand(num_points) - 1) - np.pi / 2
phi = np.random.rand(num_points) * 2 * np.pi
x = np.cos(lamd) * np.cos(phi)
y = np.cos(lamd) * np.sin(phi)
z = np.sin(lamd)
unit_sphere_positions = np.transpose([x, y, z])
rr.log_points(
    "world/points", positions=unit_sphere_positions * radius, colors=np.abs(unit_sphere_positions), radii=0.01
)

# Simple image that indicates orientation
img = np.zeros((50, 100, 3))
img[0:3, 0:3] = [255, 255, 255]
img[3:25, 0:3] = [0, 255, 0]
img[0:3, 3:25] = [255, 0, 0]

# Depth image for testing depth cloud
depth = np.ones((50, 100)) * 0.5


rr.log_view_coordinates("world", up="+Z")


def log_camera(translation: npt.ArrayLike, xyz: str, img: npt.NDArray[np.float64], forward: npt.ArrayLike) -> None:
    [height, width, _channels] = img.shape
    f_len = (height * width) ** 0.5
    # TODO(andreas): It should be possible to collapse the image path with the base path.
    cam_path = f"world/{xyz}"
    pinhole_path = f"{cam_path}/pinhole"
    rr.log_point(f"{cam_path}/indicator", position=[0, 0, 0], color=[255, 255, 255], label=xyz)
    rr.log_transform3d(cam_path, transform=rr.Translation3D(translation))
    rr.log_arrow(cam_path + "/arrow", origin=[0, 0, 0], vector=forward, color=[255, 255, 255], width_scale=0.025)
    rr.log_pinhole(
        pinhole_path,
        width=width,
        height=height,
        focal_length_px=f_len,
        camera_xyz=xyz,
    )
    rr.log_image(f"{pinhole_path}/rgb", img)
    rr.log_depth_image(f"{pinhole_path}/depth", depth)


# Log a series of pinhole cameras only differing by their view coordinates and some offset.
# Not all possible, but a fair sampling.

# All right-handed permutations of RDF:
log_camera([2, -2, 0], "RDF", img, forward=[0, 0, 1])
log_camera([2, 0, 0], "FRD", img, forward=[1, 0, 0])
log_camera([2, 2, 0], "DFR", img, forward=[0, 1, 0])

# All right-handed permutations of LUB:
log_camera([0, -2, 0], "ULB", img, forward=[0, 0, -1])
log_camera([0, 0, 0], "LBU", img, forward=[0, -1, 0])
log_camera([0, 2, 0], "BUL", img, forward=[-1, 0, 0])

# All permutations of LUF:
log_camera([-2, -2, 0], "LUF", img, forward=[0, 0, 1])
log_camera([-2, 0, 0], "FLU", img, forward=[1, 0, 0])
log_camera([-2, 2, 0], "UFL", img, forward=[0, 1, 0])


rr.script_teardown(args)
