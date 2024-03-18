#!/usr/bin/env python3
"""
Generate SDFs for arbitrary meshes and visualize the results using the Rerun SDK.

Using both traditional methods as well as the one described in the DeepSDF paper ([1]).

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

Run:
```sh
# assuming your virtual env is up
examples/python/signed_distance_fields/main.py
```
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
from timeit import default_timer as timer
from typing import cast

import mesh_to_sdf
import numpy as np
import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk
import trimesh
from download_dataset import AVAILABLE_MESHES, ensure_mesh_downloaded
from trimesh import Trimesh

CACHE_DIR = Path(os.path.dirname(__file__)) / "cache"


def log_timing_decorator(objpath: str, level: str):  # type: ignore[no-untyped-def]
    """Times the inner method using `timeit`, and logs the result using Rerun."""

    def inner(func):  # type: ignore[no-untyped-def]
        def wrapper(*args, **kwargs):  # type: ignore[no-untyped-def]
            now = timer()
            result = func(*args, **kwargs)
            elapsed_ms = (timer() - now) * 1_000.0
            rr.log(objpath, rr.TextLog(f"execution took {elapsed_ms:.1f}ms", level=level))
            return result

        return wrapper

    return inner


def read_mesh(path: Path) -> Trimesh:
    print(f"loading mesh {path}…")
    mesh = trimesh.load(path)
    return cast(Trimesh, mesh)


@log_timing_decorator("global/voxel_sdf", "DEBUG")  # type: ignore[misc]
def compute_voxel_sdf(mesh: Trimesh, resolution: int) -> npt.NDArray[np.float32]:
    print("computing voxel-based SDF")
    voxvol = np.array(mesh_to_sdf.mesh_to_voxels(mesh, voxel_resolution=resolution), dtype=np.float32)
    return voxvol


@log_timing_decorator("global/sample_sdf", "DEBUG")  # type: ignore[misc]
def compute_sample_sdf(mesh: Trimesh, num_points: int) -> tuple[npt.NDArray[np.float32], npt.NDArray[np.float32]]:
    print("computing sample-based SDF")
    points, sdf, _ = mesh_to_sdf.sample_sdf_near_surface(mesh, number_of_points=num_points, return_gradients=True)
    return (points, sdf)


@log_timing_decorator("global/log_mesh", "DEBUG")  # type: ignore[misc]
def log_mesh(path: Path, mesh: Trimesh) -> None:
    # Internally, `mesh_to_sdf` will normalize everything to a unit sphere centered around the
    # center of mass.
    # We need to compute a proper transform to map the mesh we're logging with the point clouds
    # that `mesh_to_sdf` returns.
    bs1 = mesh.bounding_sphere
    bs2 = mesh_to_sdf.scale_to_unit_sphere(mesh).bounding_sphere

    scale = bs2.scale / bs1.scale
    center = bs2.center - bs1.center * scale

    mesh3d = rr.Asset3D(path=path)
    mesh3d.transform = rr.OutOfTreeTransform3DBatch(rr.TranslationRotationScale3D(translation=center, scale=scale))
    rr.log("world/mesh", mesh3d)


def log_sampled_sdf(points: npt.NDArray[np.float32], sdf: npt.NDArray[np.float32]) -> None:
    rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)
    rr.log("world/sdf", rr.AnnotationContext([(0, "inside", (255, 0, 0)), (1, "outside", (0, 255, 0))]), static=False)
    rr.log("world/sdf/points", rr.Points3D(points, class_ids=np.array(sdf > 0, dtype=np.uint8)))

    outside = points[sdf > 0]
    rr.log(
        "world/sdf/inside/logs",
        rr.TextLog(
            f"{len(points) - len(outside)} points inside ({len(points)} total)",
            level=rr.TextLogLevel.TRACE,
        ),
    )
    rr.log(
        "world/sdf/outside/logs",
        rr.TextLog(
            f"{len(outside)} points outside ({len(points)} total)",
            level=rr.TextLogLevel.TRACE,
        ),
    )


def log_volumetric_sdf(voxvol: npt.NDArray[np.float32]) -> None:
    names = ["width", "height", "depth"]
    rr.log("tensor", rr.Tensor(voxvol, dim_names=names))


@log_timing_decorator("global/log_mesh", "DEBUG")  # type: ignore[misc]
def compute_and_log_volumetric_sdf(mesh_path: Path, mesh: Trimesh, resolution: int) -> None:
    os.makedirs(CACHE_DIR, exist_ok=True)
    basename = os.path.basename(mesh_path)
    voxvol_path = f"{CACHE_DIR}/{basename}.voxvol.{resolution}.npy"
    try:
        with open(voxvol_path, "rb") as f:
            voxvol = np.load(voxvol_path)
            rr.log("global", rr.TextLog("loading volumetric SDF from cache"))
    except Exception:
        voxvol = compute_voxel_sdf(mesh, resolution)

    log_volumetric_sdf(voxvol)

    with open(voxvol_path, "wb+") as f:
        np.save(f, voxvol)
        rr.log("global", rr.TextLog("writing volumetric SDF to cache", level=rr.TextLogLevel.DEBUG))


@log_timing_decorator("global/log_mesh", "DEBUG")  # type: ignore[misc]
def compute_and_log_sample_sdf(mesh_path: Path, mesh: Trimesh, num_points: int) -> None:
    basename = os.path.basename(mesh_path)
    points_path = f"{CACHE_DIR}/{basename}.points.{num_points}.npy"
    sdf_path = f"{CACHE_DIR}/{basename}.sdf.npy"

    os.makedirs(CACHE_DIR, exist_ok=True)
    try:
        with open(sdf_path, "rb") as f:
            sdf = np.load(sdf_path)
            rr.log("global", rr.TextLog("loading sampled SDF from cache"))
        with open(points_path, "rb") as f:
            points = np.load(points_path)
            rr.log("global", rr.TextLog("loading point cloud from cache"))
    except Exception:
        (points, sdf) = compute_sample_sdf(mesh, num_points)

    log_mesh(mesh_path, mesh)
    log_sampled_sdf(points, sdf)

    with open(points_path, "wb+") as f:
        np.save(f, points)
        rr.log("global", rr.TextLog("writing sampled SDF to cache", level=rr.TextLogLevel.DEBUG))
    with open(sdf_path, "wb+") as f:
        np.save(f, sdf)
        rr.log("global", rr.TextLog("writing point cloud to cache", level=rr.TextLogLevel.DEBUG))


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generates SDFs for arbitrary meshes and logs the results using the Rerun SDK."
    )
    parser.add_argument("--resolution", type=int, default=128, help="Specifies the resolution of the voxel volume")
    parser.add_argument(
        "--points", type=int, default=250_000, help="Specifies the number of points for the point cloud"
    )
    parser.add_argument(
        "--mesh",
        type=str,
        choices=AVAILABLE_MESHES,
        default=AVAILABLE_MESHES[0],
        help="The name of the mesh to analyze",
    )
    parser.add_argument(
        "--mesh-path",
        type=Path,
        help="Path to a mesh to analyze. If set, overrides the `--mesh` argument.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_signed_distance_fields")

    mesh_path = args.mesh_path
    if mesh_path is None:
        mesh_path = ensure_mesh_downloaded(args.mesh)
    mesh = read_mesh(mesh_path)

    compute_and_log_sample_sdf(mesh_path, mesh, args.points)

    compute_and_log_volumetric_sdf(mesh_path, mesh, args.resolution)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
