"""Log different transforms between three arrows."""
from math import pi

import rerun as rr
from rerun.datatypes import Angle, RotationAxisAngle

rr.init("rerun_example_transform3d", spawn=True)

rr.log("base", rr.Arrows3D(origins=[0, 0, 0], vectors=[0, 1, 0]))

rr.log("base/translated", rr.TranslationAndMat3x3(translation=[1, 0, 0]))
rr.log("base/translated", rr.Arrows3D(origins=[0, 0, 0], vectors=[0, 1, 0]))

rr.log(
    "base/rotated_scaled",
    rr.TranslationRotationScale3D(
        rotation=RotationAxisAngle(axis=[0, 0, 1], angle=Angle(rad=pi / 4)),
        scale=2,
    ),
)
rr.log("base/rotated_scaled", rr.Arrows3D(origins=[0, 0, 0], vectors=[0, 1, 0]))
