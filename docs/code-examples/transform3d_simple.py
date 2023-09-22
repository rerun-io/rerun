"""Log different transforms between three arrows."""
from math import pi

import rerun as rr
import rerun.experimental as rr2
from rerun.experimental import dt as rrd

rr.init("rerun_example_transform3d", spawn=True)

rr2.log("base", rr2.Arrows3D([0, 1, 0]))

rr2.log("base/translated", rrd.TranslationAndMat3x3(translation=[1, 0, 0]))
rr2.log("base/translated", rr2.Arrows3D([0, 1, 0]))

rr2.log(
    "base/rotated_scaled",
    rrd.TranslationRotationScale3D(
        rotation=rrd.RotationAxisAngle(axis=[0, 0, 1], angle=rrd.Angle(rad=pi / 4)),
        scale=2,
    ),
)
rr2.log("base/rotated_scaled", rr2.Arrows3D([0, 1, 0]))
