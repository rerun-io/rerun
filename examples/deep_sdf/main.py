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
python3 examples/deep_sdf/main.py examples/deep_sdf/dataset/avocado.glb
```
"""


import argparse
import os

import mesh_to_sdf
import numpy as np
import rerun_sdk as rerun
import trimesh

from pathlib import Path
from typing import Tuple, cast

from scipy.spatial.transform import Rotation as R
from trimesh import Trimesh
from rerun_sdk import MeshFormat


# TODO(cmc): This really should be the job of the SDK.
def get_mesh_format(mesh: Trimesh) -> MeshFormat:
    ext = Path(mesh.metadata['file_name']).suffix.lower()
    match ext:
        case ".glb":
            return MeshFormat.GLB
        # case ".gltf":
        #     return MeshFormat.GLTF
        case ".obj":
            return MeshFormat.OBJ
        case _:
            raise ValueError(f"unknown file extension: {ext}")


def read_mesh(path: Path) -> Trimesh:
    print(f"loading mesh {path}…")
    mesh = trimesh.load(path)
    return cast(Trimesh, mesh)


def compute_voxel_sdf(mesh: Trimesh, resolution: int) -> np.ndarray:
    print("computing voxel-based SDF")
    voxvol = mesh_to_sdf.mesh_to_voxels(mesh, voxel_resolution=resolution)
    return voxvol


def compute_sample_sdf(mesh: Trimesh, num_points: int) -> Tuple[np.ndarray, np.ndarray]:
    print("computing sample-based SDF")
    points, sdf, _ = mesh_to_sdf.sample_sdf_near_surface(mesh,
                                                         number_of_points=num_points,
                                                         return_gradients=True)
    return (points, sdf)


def log_mesh(path: Path, mesh: Trimesh):
    # Internally, `mesh_to_sdf` will normalize everything to a unit sphere centered around the
    # center of mass.
    # We need to compute a proper transform to map the mesh we're logging with the point clouds
    # that `mesh_to_sdf` returns.
    bs1 = mesh.bounding_sphere
    bs2 = mesh_to_sdf.scale_to_unit_sphere(mesh).bounding_sphere

    with open(path, mode='rb') as file:
        scale = bs2.scale / bs1.scale
        center = bs2.center - bs1.center * scale
        rerun.log_mesh_file("mesh", mesh_format, file.read(), space="world",
                            transform=np.array([[scale, 0, 0, center[0]],
                                                [0, scale, 0, center[1]],
                                                [0, 0, scale, center[2]]]))


def log_sampled_sdf(points: np.ndarray, sdf: np.ndarray):
    rerun.set_space_up("world", [0, 1, 0]) # TODO: depends on the mesh really
    rerun.log_points("sdf/inside",
                     points[sdf <= 0],
                     colors=np.array([255, 0, 0, 255]),
                     space="world")
    rerun.log_points("sdf/outside",
                     points[sdf > 0],
                     colors=np.array([0, 255, 0, 255]),
                     space="world")


def log_volumetric_sdf(voxvol: np.ndarray):
    names = ["width", "height", "depth"]
    rerun.log_tensor("sdf/tensor", voxvol, names=names, space="tensor")


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
    parser.add_argument('--points', type=int, default=250_000,
                        help='Specifies the number of points for the point cloud')
    parser.add_argument('path', type=Path,
                        help='Mesh to log (e.g. `dataset/avocado.glb`)')
    args = parser.parse_args()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    cachedir = Path(os.path.dirname(__file__)).joinpath("cache")
    os.makedirs(cachedir, exist_ok=True)

    path = args.path
    mesh = read_mesh(path)
    mesh_format = get_mesh_format(mesh)

    basename = os.path.basename(path)
    points_path = f"{cachedir}/{basename}.points.{args.points}.npy"
    sdf_path = f"{cachedir}/{basename}.sdf.npy"
    voxvol_path = f"{cachedir}/{basename}.voxvol.{args.resolution}.npy"

    try:
        with open(sdf_path, 'rb') as f:
            sdf = np.load(sdf_path)
        with open(points_path, 'rb') as f:
            points = np.load(points_path)
    except:
        (points, sdf) = compute_sample_sdf(mesh, args.points)

    try:
        with open(voxvol_path, 'rb') as f:
            voxvol = np.load(voxvol_path)
    except:
        voxvol = compute_voxel_sdf(mesh, args.resolution)

    log_mesh(path, mesh)
    log_sampled_sdf(points, sdf)
    log_volumetric_sdf(voxvol)

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
