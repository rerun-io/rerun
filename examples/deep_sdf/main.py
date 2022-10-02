#!/usr/bin/env python3

"""
Generate SDFs for arbitrary meshes using both traditional methods as well as the one described in
the DeepSDF paper ([1]), and visualize the results using the Rerun SDK.

[1] @InProceedings{Park_2019_CVPR,
    author = {
        Park,
        Jeong Joon and Florence,
        Peter and Straub,
        Julian and Newcombe,
        Richard and Lovegrove,
        Steven,
    },
    title = {DeepSDF: Learning Continuous Signed Distance Functions for Shape Representation},
    booktitle = {The IEEE Conference on Computer Vision and Pattern Recognition (CVPR)},
    month = {June},
    year = {2019}
}

Setup:
```sh
./examples/deep_sdf/download_dataset.py
```

Run:
```sh
# assuming your virtual env is up
# TODO(cmc): use a glb example asap
python3 examples/deep_sdf/main.py examples/deep_sdf/dataset/buddha/buddha.obj
```
"""


import argparse
import os
import sys

import mesh_to_sdf
import numpy as np
import rerun_sdk as rerun
import trimesh

from pathlib import Path
from typing import Tuple, cast

from scipy.spatial.transform import Rotation as R
from trimesh import Trimesh


def read_mesh(path: Path) -> Trimesh:
    print(f"loading mesh {path}…")
    mesh = trimesh.load(path)
    return cast(Trimesh, mesh)


def compute_voxel_sdf(mesh: Trimesh, resolution: int) -> np.ndarray:
    print("computing voxel-based SDF")
    voxvol = mesh_to_sdf.mesh_to_voxels(mesh, voxel_resolution=resolution)
    return voxvol


def compute_sample_sdf(mesh: Trimesh) -> Tuple[np.ndarray, np.ndarray]:
    print("computing sample-based SDF")
    points, sdf, _ = mesh_to_sdf.sample_sdf_near_surface(mesh,
                                                         number_of_points=250000,
                                                         return_gradients=True)
    return (points, sdf)


def log_mesh(path: Path, mesh: Trimesh, points: np.ndarray, sdf: np.ndarray):
    rerun.set_space_up("world", [0, 1, 0])
    rerun.log_points("sdf/inside",
                     points[sdf <= 0],
                     colors=np.array([255, 0, 0, 255]),
                     space="world")
    rerun.log_points("sdf/outside",
                     points[sdf > 0],
                     colors=np.array([0, 255, 0, 255]),
                     space="world")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Generates SDFs for arbitrary meshes and logs the results using the Rerun SDK.')
    parser.add_argument('--headless', action='store_true',
                        help="Don't show GUI")
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    parser.add_argument('--save', type=str, default=None,
                        help='Save data to a .rrd file at this path')
    parser.add_argument('--resolution', type=int, default=128,
                        help='Specifies the resolution of the voxel volume')
    parser.add_argument('path', type=Path, nargs='+',
                        help='Mesh(es) to log (e.g. `dataset/buddha/buddha.obj`)')
    args = parser.parse_args()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    cachedir = Path(os.path.dirname(__file__)).joinpath("cache")
    os.makedirs(cachedir, exist_ok=True)

    for path in args.path:
        # TODO(cmc): gotta match the mesh center with the point cloud center first
        # with open(path, mode='rb') as file:
        #     rerun.log_mesh_file("mesh", MeshFormat.OBJ, file.read())

        basename = os.path.basename(path)
        points_path = f"{cachedir}/{basename}.points.npy"
        sdf_path = f"{cachedir}/{basename}.sdf.npy"
        voxvol_path = f"{cachedir}/{basename}.voxvol.{args.resolution}.npy"

        mesh = read_mesh(path)

        try:
            with open(sdf_path, 'rb') as f:
                sdf = np.load(sdf_path)
            with open(points_path, 'rb') as f:
                points = np.load(points_path)
        except:
            (points, sdf) = compute_sample_sdf(mesh)

        try:
            with open(voxvol_path, 'rb') as f:
                voxvol = np.load(voxvol_path)
        except:
            voxvol = compute_voxel_sdf(mesh, args.resolution)

        log_mesh(path, mesh, points, sdf)

        names = ["width", "height", "depth"]
        rerun.log_tensor("sdf/tensor", voxvol, names=names, space="tensor")

        with open(points_path, 'wb+') as f:
            np.save(f, points)
        with open(sdf_path, 'wb+') as f:
            np.save(f, sdf)
        with open(voxvol_path, 'wb+') as f:
            np.save(f, voxvol)

    if args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()
