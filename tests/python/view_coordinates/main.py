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


rr.log_view_coordinates("world", up="+Z")


def log_camera(
    path_prefix: str, translation: npt.ArrayLike, xyz: str, img: npt.NDArray[np.float64], forward: npt.ArrayLike
) -> None:
    [height, width, _channels] = img.shape
    u_cen = width / 2
    v_cen = height / 2
    f_len = (height * width) ** 0.5
    # TODO(andreas): It should be possible to collapse the image path with the base path.
    base_path = f"world/{path_prefix}/{xyz}"
    image_path = f"{base_path}/image-{xyz}"
    rr.log_point(f"{base_path}/indicator", position=[0, 0, 0], color=[255, 255, 255], label=xyz)
    rr.log_view_coordinates(base_path, xyz=xyz)
    rr.log_transform3d(base_path, transform=rr.Translation3D(translation))
    rr.log_arrow(base_path + "/box", origin=[0, 0, 0], vector=forward, color=[255, 255, 255], width_scale=0.025)
    rr.log_pinhole(
        image_path,
        child_from_parent=[[f_len, 0, u_cen], [0, f_len, v_cen], [0, 0, 1]],
        width=width,
        height=height,
    )
    rr.log_image(image_path, img)


# Log a series of pinhole cameras only differing by their view coordinates and some offset.
# Not all possible, but a fair sampling.

# All right-handed permutations of RDF:
log_camera("rdf-perms", [2, -2, 0], "RDF", img, forward=[0, 0, 1])
log_camera("rdf-perms", [2, 0, 0], "FRD", img, forward=[1, 0, 0])
log_camera("rdf-perms", [2, 2, 0], "DFR", img, forward=[0, 1, 0])

# All right-handed permutations of LUB:
log_camera("lub-like", [0, -2, 0], "ULB", img, forward=[0, 0, -1])
log_camera("lub-like", [0, 0, 0], "LBU", img, forward=[0, -1, 0])
log_camera("lub-like", [0, 2, 0], "BUL", img, forward=[-1, 0, 0])

# All permutations of LUF:
log_camera("luf-like", [-2, -2, 0], "LUF", img, forward=[0, 0, 1])
log_camera("luf-like", [-2, 0, 0], "FLU", img, forward=[1, 0, 0])
log_camera("luf-like", [-2, 2, 0], "UFL", img, forward=[0, 1, 0])


rr.script_teardown(args)
