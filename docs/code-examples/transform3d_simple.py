"""Log different transforms between three arrows."""
from math import pi

import rerun as rr
import rerun.experimental as rr2
from rerun.experimental import dt as rrd

rr.init("rerun_example_transform3d", spawn=True)

origin = [0, 0, 0]
base_vector = [0, 1, 0]

rr.log_arrow("base", origin=origin, vector=base_vector)

rr2.log("base/translated", rrd.TranslationRotationScale3D(translation=[1, 0, 0]))

rr.log_arrow("base/translated", origin=origin, vector=base_vector)

rr2.log(
    "base/rotated_scaled",
    rrd.TranslationRotationScale3D(
        rotation=rrd.RotationAxisAngle(axis=[0, 0, 1], angle=rrd.Angle(rad=pi / 4)),
        scale=2,
    ),
)

rr.log_arrow("base/rotated_scaled", origin=origin, vector=base_vector)
