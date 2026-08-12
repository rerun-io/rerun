"""Log a few gaussian splats."""

import rerun as rr

rr.init("rerun_example_gaussian_splats3d", spawn=True)

rr.log(
    "gaussians",
    rr.GaussianSplats3D(
        centers=[[0, 0, 0], [2, 0, 0], [4, 0, 0]],
        scales=[[1.0, 0.5, 0.25], [0.5, 1.0, 0.5], [0.25, 0.5, 1.0]],
        quaternions=[
            rr.Quaternion.identity(),
            rr.Quaternion(
                xyzw=[0.0, 0.0, 0.382683, 0.923880]
            ),  # 45 degrees around Z
            rr.Quaternion.identity(),
        ],
        colors=[(255, 0, 0, 128), (0, 255, 0, 200), (0, 0, 255, 255)],
        # 15 view-dependent RGB coefficients (degrees 1-3) per splat:
        sh_coefficients=[
            [[0.5, 0.0, 0.0]] * 15,
            [[0.0, 0.5, 0.0]] * 15,
            [[0.0, 0.0, 0.5]] * 15,
        ],
    ),
)
