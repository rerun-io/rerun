"""Log a 3D Gaussian Splatting (3DGS) PLY file."""

import sys

import rerun as rr

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_splats.ply>")
    sys.exit(1)

rr.init("rerun_example_gaussian_splats3d_ply", spawn=True)

rr.log_file_from_path(sys.argv[1])
